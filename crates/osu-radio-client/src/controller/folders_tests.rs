#![allow(clippy::indexing_slicing)]
use super::*;
use std::sync::Arc;

fn controller() -> Controller {
    Controller::new(crate::ServerOptions::default(), Arc::new(|_| {}))
}
fn candidate(state: &mut Controller, path: &str, id: Option<i32>) {
    state.merge_candidate("stable".into(), path.into(), format!("{path}/osu!.db"), id);
}
#[test]
fn staging_counts_and_late_results_are_isolated_per_opening() {
    let mut state = controller();
    state.selection.state.open = true;
    candidate(&mut state, "/first", Some(7));
    candidate(&mut state, "/new", None);
    candidate(&mut state, "/new", None);
    assert_eq!(state.selection.state.rows.len(), 2);
    assert_eq!(state.selection.queue.len(), 2);
    state.toggle_selection("/new/osu!.db");
    assert_eq!(
        state.selection.state.rows[1].pending,
        Some(FolderAction::Add)
    );
    state.toggle_selection("/new/osu!.db");
    assert_eq!(state.selection.state.rows[1].pending, None);
    state.toggle_selection("/first/osu!.db");
    assert_eq!(
        state.selection.state.rows[0].pending,
        Some(FolderAction::Remove)
    );
    state.selection_event(FolderEvent::Count {
        epoch: 0,
        path: "/new/osu!.db".into(),
        result: Ok(0),
    });
    assert_eq!(state.selection.state.rows[1].count, Loading::Ready(0));
    state.selection_event(FolderEvent::Count {
        epoch: 0,
        path: "/first/osu!.db".into(),
        result: Err("unavailable".into()),
    });
    assert_eq!(
        state.selection.state.rows[0].count.error(),
        Some("unavailable")
    );
    state.retry_count("/first/osu!.db");
    assert!(state.selection.state.rows[0].count.is_pending());
    state.selection.state.applying = true;
    state.close_selection();
    state.toggle_selection("/new/osu!.db");
    assert!(state.selection.state.open);
    assert_eq!(state.selection.state.rows[1].pending, None);
    state.selection.state.applying = false;
    state.close_selection();
    state.selection.state.open = true;
    state.selection_event(FolderEvent::Count {
        epoch: 0,
        path: "/new/osu!.db".into(),
        result: Ok(42),
    });
    assert!(state.selection.state.rows.is_empty());
}

#[test]
fn partial_apply_clears_successes_and_preserves_only_failures() {
    let mut state = controller();
    state.selection.state.open = true;
    candidate(&mut state, "/first", None);
    candidate(&mut state, "/second", None);
    for path in ["/first/osu!.db", "/second/osu!.db"] {
        state.toggle_selection(path);
    }
    let folder = OsuFolder {
        id: 4,
        kind: "stable".into(),
        root_path: "/first".into(),
        marker_path: "/first/osu!.db".into(),
        label: None,
        enabled: true,
        last_scanned_at: None,
    };
    state.selection_event(FolderEvent::Applied {
        epoch: 0,
        path: folder.marker_path.clone(),
        result: Ok(Some(folder)),
    });
    state.selection_event(FolderEvent::Applied {
        epoch: 0,
        path: "/second/osu!.db".into(),
        result: Err("broken".into()),
    });
    assert_eq!(state.folders.len(), 1);
    assert_eq!(state.selection.state.rows[0].pending, None);
    assert_eq!(
        state.selection.state.rows[1].pending,
        Some(FolderAction::Add)
    );
    assert_eq!(
        state.selection.state.rows[1].error.as_deref(),
        Some("broken")
    );
    // Don't trigger connection in this source-state check.
    state.selection.changed = false;
    state.selection_event(FolderEvent::ApplyDone { epoch: 0 });
    assert!(state.selection.state.open);
    state.toggle_selection("/second/osu!.db");
    state.selection_event(FolderEvent::ApplyDone { epoch: 0 });
    assert!(!state.selection.state.open);
}

#[tokio::test]
async fn scoped_discovery_stages_only_unique_new_installations_and_rejects_old_chooser_results() {
    let mut state = controller();
    state.selection.state.open = true;
    for (scan, paths) in [
        (1, vec!["/unique"]),
        (2, vec!["/left", "/right"]),
        (3, vec!["/unique"]),
    ] {
        let task = tokio::spawn(std::future::pending::<()>());
        state.selection.scans.insert(
            scan,
            Scan {
                task: task.abort_handle(),
                scoped: true,
                markers: Vec::new(),
            },
        );
        for path in paths {
            state.selection_event(FolderEvent::Candidate {
                epoch: 0,
                scan,
                event: FolderDiscoveryEvent::Candidate {
                    kind: "stable".into(),
                    root_path: path.into(),
                    marker_path: format!("{path}/osu!.db"),
                    registered_id: None,
                },
            });
        }
        state.selection_event(FolderEvent::Discovered {
            epoch: 0,
            scan,
            result: Ok(()),
        });
        task.abort();
        let _ = task.await;
    }
    assert_eq!(state.selection.state.rows.len(), 3);
    assert_eq!(
        state.selection.state.rows[0].pending,
        Some(FolderAction::Add)
    );
    assert_eq!(state.selection.state.rows[1].pending, None);
    assert_eq!(state.selection.state.rows[2].pending, None);
    state.selection.state.picking = true;
    state.close_selection();
    state.selection.state.open = true;
    state.selection.state.picking = true;
    state.selection_picked(0, Some("/old/chooser".into()));
    assert!(state.selection.state.picking);
    assert!(state.selection.scans.is_empty());
}

#[test]
fn refresh_staging_replaces_removal_and_cancels_only_the_same_action() {
    let mut state = controller();
    state.selection.state.open = true;
    candidate(&mut state, "/registered", Some(7));
    candidate(&mut state, "/new", None);
    let path = "/registered/osu!.db";
    state.refresh_selection(path);
    assert_eq!(
        state.selection.state.rows[0].pending,
        Some(FolderAction::Refresh)
    );
    state.toggle_selection(path);
    assert_eq!(
        state.selection.state.rows[0].pending,
        Some(FolderAction::Remove)
    );
    state.refresh_selection(path);
    assert_eq!(
        state.selection.state.rows[0].pending,
        Some(FolderAction::Refresh)
    );
    state.refresh_selection(path);
    assert_eq!(state.selection.state.rows[0].pending, None);
    state.refresh_selection("/new/osu!.db");
    assert_eq!(state.selection.state.rows[1].pending, None);
    state.refresh_selection(path);
    state.selection.state.applying = true;
    state.toggle_selection(path);
    state.refresh_selection(path);
    assert_eq!(
        state.selection.state.rows[0].pending,
        Some(FolderAction::Refresh)
    );
    state.selection_event(FolderEvent::Applied {
        epoch: 0,
        path: path.into(),
        result: Err("broken source".into()),
    });
    assert_eq!(
        state.selection.state.rows[0].pending,
        Some(FolderAction::Refresh)
    );
    assert_eq!(
        state.selection.state.rows[0].error.as_deref(),
        Some("broken source")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn successful_apply_refreshes_playlists_even_when_library_loading_fails() {
    let (_directory, session) = super::super::tests::test_session().await;
    let (mut state, _) = super::super::tests::controller();
    state.session = Some(session);
    state.selection.state.open = true;
    state.playlists.view.active_id = Some(7);
    candidate(&mut state, "/registered", Some(4));
    state.refresh_selection("/registered/osu!.db");
    state.selection_event(FolderEvent::Applied {
        epoch: 0,
        path: "/registered/osu!.db".into(),
        result: Ok(Some(OsuFolder {
            id: 4,
            kind: "stable".into(),
            root_path: "/registered".into(),
            marker_path: "/registered/osu!.db".into(),
            label: None,
            enabled: true,
            last_scanned_at: Some("2026-10-04T00:00:00Z".into()),
        })),
    });
    state.selection_event(FolderEvent::ApplyDone { epoch: 0 });
    assert!(!state.selection.state.open);
    assert!(state.playlists.view.loading);
    assert_eq!(
        state.tasks.len(),
        4,
        "folder, library, playlist list and active detail refresh directly"
    );
    state.complete(Completed::Library {
        request: state.library_request,
        invalidate_artwork: true,
        result: Err("library unavailable".into()),
    });
    assert!(state.playlists.view.loading);
    assert_eq!(state.playlists.view.active_id, Some(7));
    state.tasks.abort_all();
    while state.tasks.join_next().await.is_some() {}
    state.session.take().unwrap().shutdown().await.unwrap();
}
