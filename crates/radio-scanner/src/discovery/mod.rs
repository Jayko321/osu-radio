//! osu! installation discovery.
//!
//! Discovery runs in tiers, cheapest first: [`DiscoveryDepth::Known`] probes exact
//! per-OS candidate paths, [`DiscoveryDepth::Shallow`] sweeps the scan roots to a
//! bounded depth, and [`DiscoveryDepth::Full`] sweeps them without a limit.
//!
//! Markers are handed to the caller as they are found rather than collected into a
//! batch, so a caller can render them live and stop the walk early via
//! [`DiscoveryOptions::limit`] or by breaking out of the sink.

mod known;
mod sweep;

use std::{
    collections::HashSet,
    num::NonZeroUsize,
    ops::ControlFlow,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use radio_core::{OsuKind, OsuMarker};
use tokio::sync::mpsc;

pub use sweep::system_scan_roots;

/// How much of the filesystem discovery is allowed to traverse.
///
/// Each variant names the *highest* tier that runs; the cheaper tiers below it always
/// run first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum DiscoveryDepth {
    /// Probe known candidate paths only. No directory traversal at all.
    Known,
    /// Known paths, then a depth-limited sweep of the scan roots.
    Shallow,
    /// Known paths, a shallow sweep, then an unbounded sweep of the scan roots.
    #[default]
    Full,
}

/// Default depth limit for the shallow sweep tier.
pub const DEFAULT_SHALLOW_MAX_DEPTH: usize = 4;

const DISCOVERY_CHANNEL_CAPACITY: usize = 16;

/// Inputs that constrain a discovery run.
#[derive(Debug, Clone)]
pub struct DiscoveryOptions {
    /// Roots to search. Empty means "use the OS defaults": every mounted drive on
    /// Windows, `/` elsewhere, plus the home directory for the sweep tiers.
    pub roots: Vec<PathBuf>,
    /// Emit only markers of this kind. Applied during the walk, so it interacts
    /// correctly with `limit`.
    pub kind: Option<OsuKind>,
    /// Stop once this many markers have been emitted.
    pub limit: Option<NonZeroUsize>,
    /// Highest tier to run.
    pub depth: DiscoveryDepth,
    /// Depth limit for the shallow sweep tier.
    pub shallow_max_depth: usize,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            kind: None,
            limit: None,
            depth: DiscoveryDepth::default(),
            shallow_max_depth: DEFAULT_SHALLOW_MAX_DEPTH,
        }
    }
}

impl DiscoveryOptions {
    fn sweep_roots(&self) -> Vec<PathBuf> {
        if !self.roots.is_empty() {
            return dedup_paths(self.roots.clone());
        }

        let mut roots = system_scan_roots();
        if let Some(home) = known::home_dir() {
            roots.push(home);
        }

        dedup_paths(roots)
    }
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

/// Runs discovery synchronously, invoking `sink` once per unique marker.
///
/// Returns as soon as `sink` returns [`ControlFlow::Break`] or
/// [`DiscoveryOptions::limit`] is reached. This blocks the calling thread; use
/// [`discover`] from async code.
pub fn find_osu_markers_with<F>(options: &DiscoveryOptions, sink: F)
where
    F: FnMut(OsuMarker) -> ControlFlow<()> + Send,
{
    let collector = Collector::new(sink, options);

    for (kind, marker_path) in known::known_candidates(&options.roots) {
        if collector.offer(kind, marker_path).is_break() {
            return;
        }
    }

    if options.depth == DiscoveryDepth::Known || collector.stopped() {
        return;
    }

    let roots = options.sweep_roots();
    sweep::sweep(&roots, Some(options.shallow_max_depth), &collector);

    if options.depth == DiscoveryDepth::Shallow || collector.stopped() {
        return;
    }

    sweep::sweep(&roots, None, &collector);
}

/// Runs discovery synchronously and collects every marker it finds.
#[must_use]
pub fn find_osu_markers(options: &DiscoveryOptions) -> Vec<OsuMarker> {
    let mut markers = Vec::new();
    find_osu_markers_with(options, |marker| {
        markers.push(marker);
        ControlFlow::Continue(())
    });

    markers
}

/// A discovery run in progress, yielding markers as they are found.
///
/// Dropping the stream cancels the walk.
#[derive(Debug)]
pub struct Discovery {
    receiver: mpsc::Receiver<OsuMarker>,
}

impl Discovery {
    /// Returns the next discovered marker, or `None` once discovery is finished.
    pub async fn next(&mut self) -> Option<OsuMarker> {
        self.receiver.recv().await
    }

    /// Drains the stream into a vector.
    pub async fn collect(mut self) -> Vec<OsuMarker> {
        let mut markers = Vec::new();
        while let Some(marker) = self.next().await {
            markers.push(marker);
        }

        markers
    }
}

/// Starts a discovery run on a blocking thread and streams the markers it finds.
#[must_use]
pub fn discover(options: DiscoveryOptions) -> Discovery {
    let (sender, receiver) = mpsc::channel(DISCOVERY_CHANNEL_CAPACITY);

    tokio::task::spawn_blocking(move || {
        find_osu_markers_with(&options, |marker| match sender.blocking_send(marker) {
            Ok(()) => ControlFlow::Continue(()),
            Err(_) => ControlFlow::Break(()),
        });
    });

    Discovery { receiver }
}

struct CollectorState<F> {
    sink: F,
    seen: HashSet<PathBuf>,
    remaining: Option<usize>,
}

pub(crate) struct Collector<F> {
    state: Mutex<CollectorState<F>>,
    kind: Option<OsuKind>,
    stop: AtomicBool,
}

impl<F> Collector<F>
where
    F: FnMut(OsuMarker) -> ControlFlow<()> + Send,
{
    fn new(sink: F, options: &DiscoveryOptions) -> Self {
        Self {
            state: Mutex::new(CollectorState {
                sink,
                seen: HashSet::new(),
                remaining: options.limit.map(NonZeroUsize::get),
            }),
            kind: options.kind,
            stop: AtomicBool::new(false),
        }
    }

    pub(crate) fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    pub(crate) fn offer(&self, kind: OsuKind, marker_path: PathBuf) -> ControlFlow<()> {
        if self.stopped() {
            return ControlFlow::Break(());
        }

        if self.kind.is_some_and(|wanted| wanted != kind) {
            return ControlFlow::Continue(());
        }

        let Some(root_path) = marker_path.parent().map(Path::to_path_buf) else {
            return ControlFlow::Continue(());
        };

        // Only a dedup key, so the emitted marker keeps the readable path.
        let key = std::fs::canonicalize(&marker_path).unwrap_or_else(|_| marker_path.clone());

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !state.seen.insert(key) {
            return ControlFlow::Continue(());
        }

        let flow = (state.sink)(OsuMarker {
            kind,
            marker_path,
            root_path,
        });

        let exhausted = state.remaining.as_mut().is_some_and(|remaining| {
            *remaining = remaining.saturating_sub(1);
            *remaining == 0
        });
        drop(state);

        if exhausted {
            self.stop.store(true, Ordering::Relaxed);
            return ControlFlow::Break(());
        }

        if flow.is_break() {
            self.stop.store(true, Ordering::Relaxed);
        }

        flow
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, num::NonZeroUsize, path::Path};

    use radio_core::OsuKind;
    use tempfile::TempDir;

    use super::{DiscoveryDepth, DiscoveryOptions, discover, find_osu_markers};

    fn fixture() -> TempDir {
        let temp = TempDir::new().expect("temp dir");
        write_marker(&temp.path().join("osu").join("client.realm"));
        write_marker(&temp.path().join("a/b/c/d/e/osu!").join("osu!.db"));

        temp
    }

    fn write_marker(path: &Path) {
        fs::create_dir_all(path.parent().expect("marker parent")).expect("create marker dir");
        fs::write(path, b"").expect("write marker");
    }

    fn options(temp: &TempDir) -> DiscoveryOptions {
        DiscoveryOptions {
            roots: vec![temp.path().to_path_buf()],
            ..DiscoveryOptions::default()
        }
    }

    fn marker_paths(markers: &[radio_core::OsuMarker]) -> Vec<std::path::PathBuf> {
        let mut paths = markers
            .iter()
            .map(|marker| marker.marker_path.clone())
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    #[test]
    fn finds_markers_under_explicit_root() {
        let temp = fixture();
        let markers = find_osu_markers(&options(&temp));

        assert_eq!(markers.len(), 2, "{markers:?}");
        assert!(markers.iter().any(|marker| marker.kind == OsuKind::Lazer));
        assert!(markers.iter().any(|marker| marker.kind == OsuKind::Stable));
    }

    #[test]
    fn stops_after_limit() {
        let temp = fixture();
        let markers = find_osu_markers(&DiscoveryOptions {
            limit: NonZeroUsize::new(1),
            ..options(&temp)
        });

        assert_eq!(markers.len(), 1, "{markers:?}");
    }

    #[test]
    fn filters_by_kind_during_the_walk() {
        let temp = fixture();
        let markers = find_osu_markers(&DiscoveryOptions {
            kind: Some(OsuKind::Stable),
            ..options(&temp)
        });

        assert_eq!(markers.len(), 1, "{markers:?}");
        assert_eq!(markers[0].kind, OsuKind::Stable);
    }

    #[test]
    fn known_depth_finds_candidates_without_walking() {
        let temp = fixture();
        let markers = find_osu_markers(&DiscoveryOptions {
            depth: DiscoveryDepth::Known,
            ..options(&temp)
        });

        assert_eq!(markers.len(), 1, "{markers:?}");
        assert_eq!(markers[0].kind, OsuKind::Lazer);
    }

    #[test]
    fn shallow_depth_skips_deeply_nested_markers() {
        let temp = fixture();
        let markers = find_osu_markers(&DiscoveryOptions {
            depth: DiscoveryDepth::Shallow,
            shallow_max_depth: 2,
            ..options(&temp)
        });

        assert!(
            markers.iter().all(|marker| marker.kind == OsuKind::Lazer),
            "{markers:?}"
        );
    }

    #[test]
    fn does_not_emit_duplicates_across_tiers() {
        let temp = fixture();
        let paths = marker_paths(&find_osu_markers(&options(&temp)));

        let mut deduped = paths.clone();
        deduped.dedup();

        assert_eq!(deduped, paths);
    }

    #[tokio::test]
    async fn discover_stream_yields_the_same_markers_as_the_sync_core() {
        let temp = fixture();
        let expected = marker_paths(&find_osu_markers(&options(&temp)));
        let streamed = marker_paths(&discover(options(&temp)).collect().await);

        assert_eq!(streamed, expected);
    }
}
