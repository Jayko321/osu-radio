//! Bounded, toolkit-free media pipeline state. Decoders acknowledge delivery explicitly.
use super::workflow::{Effect, Workflow};
use super::{AppUpdate, Completed, Duration, MediaTicket, Track};
use osu_radio_media_controls::{Artwork, ArtworkKey};
use std::collections::{HashMap, HashSet, VecDeque};
use tokio::task::AbortHandle;

pub(crate) struct MediaJob {
    pub(crate) ticket: MediaTicket,
    pub(crate) cover: Option<i32>,
    pub(crate) decoding: bool,
    pub(crate) artwork_pending: bool,
    pub(crate) duration_pending: bool,
    pub(crate) tasks: Vec<AbortHandle>,
}
struct SystemArtwork {
    audio_id: i32,
    artwork: Artwork,
    ticket: Option<MediaTicket>,
}
#[derive(Default)]
pub(crate) struct MediaScheduler {
    system: Option<SystemArtwork>,
    pub(crate) gui_hidden: bool,
    pub(crate) generation: u64,
    pub(crate) serial: u64,
    pub(crate) queue: VecDeque<(i32, bool)>,
    pub(crate) visible: Option<Vec<(i32, bool)>>,
    pub(crate) selected_artwork_missing: Option<bool>,
    pub(crate) queue_media: HashMap<i32, bool>,
    pub(crate) visible_queue: Vec<(crate::models::QueueEntryId, bool)>,
    pub(crate) jobs: HashMap<u64, MediaJob>,
    pub(crate) durations: HashMap<i32, Option<Duration>>,
    pub(crate) unavailable: HashSet<i32>,
}
impl MediaScheduler {
    pub(crate) fn sync_system_artwork(&mut self, current: Option<(i32, ArtworkKey)>) {
        if self
            .system
            .as_ref()
            .map(|system| (system.audio_id, system.artwork.key))
            == current
        {
            return;
        }
        if let Some(ticket) = self.system.take().and_then(|system| system.ticket)
            && let Some(job) = self
                .jobs
                .get_mut(&ticket.serial)
                .filter(|job| job.ticket == ticket)
        {
            // A new launch cannot adopt the previous launch's in-flight response.
            for task in job.tasks.drain(..) {
                task.abort();
            }
            job.artwork_pending = false;
            job.duration_pending = false;
            if !job.decoding {
                self.jobs.remove(&ticket.serial);
            }
        }
        self.system = current.map(|(audio_id, key)| SystemArtwork {
            audio_id,
            artwork: Artwork { key, bytes: None },
            ticket: None,
        });
    }

    pub(crate) fn system_artwork(&self, current: Option<(i32, ArtworkKey)>) -> Option<Artwork> {
        self.system
            .as_ref()
            .filter(|system| Some((system.audio_id, system.artwork.key)) == current)
            .map(|system| system.artwork.clone())
    }

    fn system_missing(&self, id: i32) -> bool {
        self.system.as_ref().is_some_and(|system| {
            system.audio_id == id
                && system.artwork.bytes.is_none()
                && !self.unavailable.contains(&system.artwork.key.cover_id)
        })
    }

    fn requested_cover(&self, id: i32, missing: bool, input: &MediaInput<'_>) -> Option<i32> {
        if self.system_missing(id) {
            self.system
                .as_ref()
                .map(|system| system.artwork.key.cover_id)
        } else {
            input
                .track(id)
                .and_then(|track| track.cover_beatmap_id)
                .filter(|cover| missing && !self.unavailable.contains(cover))
        }
    }

    fn share_system_request(&mut self, ticket: MediaTicket, cover: Option<i32>) {
        if let Some(system) = &mut self.system
            && system.audio_id == ticket.audio_id
            && system.artwork.key.generation == ticket.generation
            && Some(system.artwork.key.cover_id) == cover
            && system.artwork.bytes.is_none()
            && system.ticket.is_none()
        {
            system.ticket = Some(ticket);
        }
    }

    pub(crate) fn prioritize_selected(&mut self, id: Option<i32>) {
        if let Some(id) = id
            && let Some(index) = self.queue.iter().position(|(queued, _)| *queued == id)
            && let Some(request) = self.queue.remove(index)
        {
            self.queue.push_front(request);
        }
    }
    pub(crate) fn restore_durations(&self, tracks: &mut [Track]) {
        for track in tracks {
            if let Some(duration) = self.durations.get(&track.audio_source_id) {
                track.duration = *duration;
            }
        }
    }
    pub(crate) fn advance(&mut self, invalidate: bool, has_selection: bool) {
        self.system = None;
        self.generation = self.generation.wrapping_add(1);
        self.queue.clear();
        if let Some(visible) = &mut self.visible {
            visible.clear();
        }
        for job in self.jobs.values_mut() {
            for task in job.tasks.drain(..) {
                task.abort();
            }
            job.artwork_pending = false;
            job.duration_pending = false;
        }
        // A started decoder retains its pipeline slot until the adapter acknowledges it.
        self.jobs.retain(|_, job| job.decoding);
        if invalidate {
            self.durations.clear();
            self.unavailable.clear();
            self.selected_artwork_missing = has_selection.then_some(true);
            for (_, missing) in &mut self.visible_queue {
                *missing = true;
            }
            for missing in self.queue_media.values_mut() {
                *missing = true;
            }
        }
    }
    pub(crate) fn finish(&mut self, ticket: MediaTicket) {
        if self.jobs.get(&ticket.serial).is_some_and(|job| {
            job.ticket == ticket && !job.artwork_pending && !job.duration_pending && !job.decoding
        }) {
            self.jobs.remove(&ticket.serial);
        }
    }
}

impl MediaScheduler {
    pub(crate) fn media_wanted(
        &self,
        id: i32,
        input: &MediaInput<'_>,
        _work: &mut Workflow<'_>,
    ) -> bool {
        self.system
            .as_ref()
            .is_some_and(|system| system.audio_id == id)
            || self.gui_media_wanted(id, input)
    }

    fn gui_media_wanted(&self, id: i32, input: &MediaInput<'_>) -> bool {
        if self.gui_hidden {
            return false;
        }
        input
            .player
            .selected_track
            .as_ref()
            .is_some_and(|track| track.audio_source_id == id)
            || (input.upcoming.is_some_and(|queue| queue.open)
                && self.queue_media.contains_key(&id))
            || self.visible.as_ref().map_or_else(
                || {
                    input.library.indices.contains_key(&id)
                        || input
                            .playlists
                            .tracks
                            .iter()
                            .any(|row| row.track.audio_source_id == id)
                },
                |visible| visible.iter().any(|(audio_id, _)| *audio_id == id),
            )
    }
    pub(crate) fn set_visible_media(
        &mut self,
        visible: Vec<(i32, bool)>,
        input: &MediaInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let mut snapshot = Vec::<(i32, bool)>::new();
        for (id, missing) in visible {
            if id < 0 || input.track(id).is_none() {
                continue;
            }
            if let Some((_, old_missing)) = snapshot.iter_mut().find(|(old_id, _)| *old_id == id) {
                *old_missing |= missing;
            } else {
                snapshot.push((id, missing));
            }
        }
        self.visible = Some(snapshot);
        self.reconcile_media(input, work);
    }
    pub(crate) fn reconcile_media(&mut self, input: &MediaInput<'_>, work: &mut Workflow<'_>) {
        if let Some(queue) = input.upcoming {
            self.visible_queue
                .retain(|(id, _)| queue.open && queue.view.entries.iter().any(|row| row.id == *id));
            self.queue_media.clear();
            for (id, missing) in &self.visible_queue {
                if let Some(audio_id) = queue
                    .view
                    .entries
                    .iter()
                    .find(|row| row.id == *id)
                    .and_then(|row| row.audio_source_id)
                {
                    *self.queue_media.entry(audio_id).or_insert(false) |= *missing;
                }
            }
        }
        let unwanted: Vec<_> = self
            .jobs
            .values()
            .filter(|job| {
                job.ticket.generation != self.generation
                    || !self.media_wanted(job.ticket.audio_id, input, work)
            })
            .map(|job| job.ticket.serial)
            .collect();
        for serial in unwanted {
            if let Some(job) = self.jobs.get_mut(&serial) {
                for task in job.tasks.drain(..) {
                    task.abort();
                }
                job.artwork_pending = false;
                job.duration_pending = false;
                if !job.decoding {
                    self.jobs.remove(&serial);
                }
            }
        }
        let queued = std::mem::take(&mut self.queue);
        if let Some(visible) = self.visible.clone() {
            for (id, missing) in visible {
                self.enqueue_media(id, missing, input, work);
            }
        } else {
            for (id, missing) in queued {
                if self.media_wanted(id, input, work) {
                    self.enqueue_media(id, missing, input, work);
                }
            }
        }
        if input.upcoming.is_some_and(|queue| queue.open) {
            for (id, missing) in self.queue_media.clone() {
                self.enqueue_media(id, missing, input, work);
            }
        }
        if let Some(id) = input
            .player
            .selected_track
            .as_ref()
            .map(|track| track.audio_source_id)
            && let Some(missing) = self.selected_artwork_missing
        {
            self.enqueue_media(id, missing, input, work);
        }
        if let Some(id) = self.system.as_ref().map(|system| system.audio_id) {
            self.enqueue_media(id, false, input, work);
        }
    }
    pub(crate) fn request_media(
        &mut self,
        id: i32,
        missing: bool,
        input: &MediaInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if self.gui_hidden || id < 0 || input.track(id).is_none() {
            return;
        }
        if input
            .player
            .selected_track
            .as_ref()
            .is_some_and(|track| track.audio_source_id == id)
        {
            self.selected_artwork_missing = Some(missing);
        }
        if self.media_wanted(id, input, work) {
            self.enqueue_media(id, missing, input, work);
        }
    }
    pub(crate) fn enqueue_media(
        &mut self,
        id: i32,
        missing: bool,
        input: &MediaInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        if (self.gui_hidden && !self.system_missing(id)) || id < 0 || input.track(id).is_none() {
            return;
        }
        let requested_cover = self.requested_cover(id, missing, input);
        if let Some((ticket, previous_cover, active_artwork)) = self
            .jobs
            .values()
            .find(|job| {
                job.ticket.audio_id == id
                    && job.ticket.generation == self.generation
                    && !(self.system_missing(id) && job.decoding)
            })
            .map(|job| (job.ticket, job.cover, job.artwork_pending || job.decoding))
        {
            if active_artwork && previous_cover == requested_cover {
                self.share_system_request(ticket, previous_cover);
            }
            if let Some(cover) = requested_cover.filter(|_| !active_artwork) {
                if previous_cover.is_some() {
                    // A second delivery needs a fresh ticket so a repeated old ACK cannot release it.
                    if let Some(job) = self.jobs.remove(&ticket.serial) {
                        for task in job.tasks {
                            task.abort();
                        }
                    }
                } else {
                    if let Some(api) = work.api.clone() {
                        let task = work.task(async move {
                            Completed::MediaArtwork {
                                ticket,
                                bytes: api.cover(cover).await.ok().flatten(),
                            }
                        });
                        if let Some(job) = self.jobs.get_mut(&ticket.serial) {
                            job.cover = Some(cover);
                            job.artwork_pending = true;
                            job.tasks.push(task);
                        }
                        self.share_system_request(ticket, Some(cover));
                    }
                    return;
                }
            } else {
                return;
            }
        }
        let selected = input
            .player
            .selected_track
            .as_ref()
            .map(|track| track.audio_source_id)
            .or(input.library.selected)
            == Some(id);
        if let Some(index) = self.queue.iter().position(|(queued, _)| *queued == id) {
            if let Some(request) = self.queue.get_mut(index) {
                request.1 |= missing;
            }
            if selected && let Some(request) = self.queue.remove(index) {
                self.queue.push_front(request);
            }
            return;
        }
        if requested_cover.is_none() && self.durations.contains_key(&id) {
            return;
        }
        if selected {
            self.queue.push_front((id, missing));
        } else {
            self.queue.push_back((id, missing));
        }
    }
    pub(crate) fn start_media(&mut self, input: &MediaInput<'_>, work: &mut Workflow<'_>) {
        if input.library.status.loading {
            return;
        }
        let Some(api) = work.api.clone() else {
            return;
        };
        while self.jobs.len() < 4 {
            let Some((id, missing)) = self.queue.pop_front() else {
                break;
            };
            if !self.media_wanted(id, input, work) {
                continue;
            }
            if input.track(id).is_none() {
                continue;
            }
            let cover = self.requested_cover(id, missing, input);
            let duration_requested = !self.durations.contains_key(&id);
            if cover.is_none() && !duration_requested {
                continue;
            }
            self.serial = self.serial.wrapping_add(1);
            let ticket = MediaTicket {
                generation: self.generation,
                audio_id: id,
                serial: self.serial,
            };
            let mut tasks = Vec::new();
            if let Some(cover_id) = cover {
                let api = api.clone();
                tasks.push(work.task(async move {
                    Completed::MediaArtwork {
                        ticket,
                        bytes: api.cover(cover_id).await.ok().flatten(),
                    }
                }));
            }
            if duration_requested {
                let api = api.clone();
                tasks.push(work.task(async move {
                    Completed::MediaDuration {
                        ticket,
                        duration: api
                            .audio_duration(id)
                            .await
                            .ok()
                            .and_then(|result| result.duration_ms),
                    }
                }));
            }
            self.jobs.insert(
                ticket.serial,
                MediaJob {
                    ticket,
                    cover,
                    decoding: false,
                    artwork_pending: cover.is_some(),
                    duration_pending: duration_requested,
                    tasks,
                },
            );
            self.share_system_request(ticket, cover);
        }
    }
    pub(crate) fn artwork_completed(
        &mut self,
        ticket: MediaTicket,
        bytes: Option<Vec<u8>>,
        input: &MediaInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let gui_wanted = self.gui_media_wanted(ticket.audio_id, input);
        let Some(job) = self
            .jobs
            .get_mut(&ticket.serial)
            .filter(|job| job.ticket == ticket && job.artwork_pending)
        else {
            return;
        };
        job.artwork_pending = false;
        if ticket.generation == self.generation {
            if let Some(system) = &mut self.system
                && system.ticket == Some(ticket)
                && system.artwork.key.generation == ticket.generation
                && Some(system.artwork.key.cover_id) == job.cover
            {
                system.ticket = None;
                system.artwork.bytes = bytes
                    .as_ref()
                    .filter(|bytes| !bytes.is_empty())
                    .map(|bytes| std::sync::Arc::from(bytes.as_slice()));
            }
            match (job.cover, bytes) {
                (Some(cover_id), Some(bytes)) if !bytes.is_empty() => {
                    if !gui_wanted {
                        self.finish(ticket);
                        return;
                    }
                    job.decoding = true;
                    work.emit(AppUpdate::Artwork {
                        ticket,
                        cover_id,
                        bytes,
                    });
                }
                (Some(cover), _) => {
                    self.unavailable.insert(cover);
                }
                _ => {}
            }
        }
        self.finish(ticket);
    }
    pub(crate) fn duration_completed(
        &mut self,
        ticket: MediaTicket,
        duration: Option<u64>,
        _input: &MediaInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        let Some(job) = self
            .jobs
            .get_mut(&ticket.serial)
            .filter(|job| job.ticket == ticket && job.duration_pending)
        else {
            return;
        };
        job.duration_pending = false;
        if ticket.generation == self.generation {
            let duration = duration.map(Duration::from_millis);
            self.durations.insert(ticket.audio_id, duration);
            work.effect(Effect::DurationChanged {
                audio_id: ticket.audio_id,
                duration,
            });
        }
        self.finish(ticket);
    }
}

pub(crate) struct MediaInput<'a> {
    pub(crate) library: &'a super::LibraryViewModel,
    pub(crate) playlists: &'a super::PlaylistsViewModel,
    pub(crate) player: &'a super::PlayerViewModel,
    pub(crate) upcoming: Option<&'a super::QueueViewModel>,
}
impl MediaInput<'_> {
    pub(crate) fn track(&self, id: i32) -> Option<&Track> {
        if self.playlists.view.showing_detail()
            && let Some(track) = self
                .playlists
                .selected_track()
                .filter(|track| track.audio_source_id == id)
                .or_else(|| {
                    self.playlists
                        .tracks
                        .iter()
                        .find(|row| row.track.audio_source_id == id)
                        .map(|row| &row.track)
                })
        {
            return Some(track);
        }
        self.player
            .current_track
            .as_ref()
            .filter(|track| track.audio_source_id == id)
            .or_else(|| {
                self.library
                    .indices
                    .get(&id)
                    .and_then(|index| self.library.tracks.get(*index))
            })
            .or_else(|| {
                self.upcoming.and_then(|queue| {
                    queue
                        .view
                        .entries
                        .iter()
                        .find(|row| row.audio_source_id == Some(id))
                        .map(|row| &row.track)
                })
            })
            .or_else(|| {
                self.player
                    .selected_track
                    .as_ref()
                    .filter(|track| track.audio_source_id == id)
            })
    }
}

impl MediaScheduler {
    pub(crate) fn discarded(&mut self, ticket: MediaTicket) {
        if let Some(job) = self
            .jobs
            .get_mut(&ticket.serial)
            .filter(|job| job.ticket == ticket)
        {
            job.decoding = false;
        }
        self.finish(ticket);
    }
    pub(crate) fn installed(
        &mut self,
        ticket: MediaTicket,
        available: bool,
        selected_id: Option<i32>,
        queue: &super::QueueView,
    ) {
        if let Some(job) = self.jobs.get_mut(&ticket.serial)
            && job.ticket == ticket
            && job.decoding
        {
            job.decoding = false;
            if ticket.generation == self.generation && available {
                if selected_id == Some(ticket.audio_id) {
                    self.selected_artwork_missing = Some(false);
                }
                // A shared audio delivery satisfies every visible duplicate.
                if let Some(missing) = self.queue_media.get_mut(&ticket.audio_id) {
                    *missing = false;
                }
                for (id, missing) in &mut self.visible_queue {
                    if queue
                        .entries
                        .iter()
                        .any(|row| row.id == *id && row.audio_source_id == Some(ticket.audio_id))
                    {
                        *missing = false;
                    }
                }
                if let Some(visible) = &mut self.visible {
                    for (id, missing) in visible {
                        if *id == ticket.audio_id {
                            *missing = false;
                        }
                    }
                }
            }
            if ticket.generation == self.generation
                && !available
                && let Some(cover) = job.cover
            {
                self.unavailable.insert(cover);
            }
            self.finish(ticket);
        }
    }
}

impl MediaScheduler {
    pub(crate) fn close_queue_demand(&mut self) {
        self.queue_media.clear();
        self.visible_queue.clear();
    }
    pub(crate) fn set_visible_queue_media(
        &mut self,
        visible: Vec<(crate::models::QueueEntryId, bool)>,
        input: &MediaInput<'_>,
        work: &mut Workflow<'_>,
    ) {
        self.visible_queue = visible;
        self.reconcile_media(input, work);
    }
    pub(crate) fn remember_duration(&mut self, audio_id: i32, duration: Option<Duration>) {
        self.durations.insert(audio_id, duration);
    }
    pub(crate) fn selection_changed(&mut self, previous: Option<&Track>, next: Option<&Track>) {
        if previous.map(|track| (track.audio_source_id, track.cover_beatmap_id))
            != next.map(|track| (track.audio_source_id, track.cover_beatmap_id))
        {
            self.selected_artwork_missing = None;
        }
    }
}
