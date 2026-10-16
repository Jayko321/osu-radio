use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use radio_services::{QueueCheck, model::SourceType};
use tokio::{sync::watch, task::JoinSet};

use crate::{audio_file::open_audio_file, state::AppState};

const CHECK_INTERVAL: Duration = Duration::from_secs(1);
const MAX_CHECKS: usize = 4;

pub(crate) struct QueueCheckWorker {
    stop: watch::Sender<bool>,
    supervisor: Option<tokio::task::JoinHandle<()>>,
}

impl QueueCheckWorker {
    pub(crate) fn start(state: AppState) -> Self {
        Self::start_with_check(state, |check| match &check.source {
            SourceType::Local(path) | SourceType::Copied(path) => {
                open_audio_file(std::path::Path::new(path)).is_ok()
            }
            SourceType::Online(_) => false,
        })
    }

    fn start_with_check(
        state: AppState,
        check_file: impl Fn(&QueueCheck) -> bool + Send + Sync + 'static,
    ) -> Self {
        let (stop, stopped) = watch::channel(false);
        Self {
            stop,
            supervisor: Some(tokio::spawn(run(state, stopped, Arc::new(check_file)))),
        }
    }

    pub(crate) async fn shutdown(mut self) -> Result<()> {
        self.stop.send_replace(true);
        if let Some(supervisor) = self.supervisor.take() {
            supervisor.await.context("Queue check supervisor failed")?;
        }
        Ok(())
    }
}

impl Drop for QueueCheckWorker {
    fn drop(&mut self) {
        // The supervisor drains blocking operations; aborting them cannot stop file I/O.
        self.stop.send_replace(true);
    }
}

async fn run(
    state: AppState,
    mut stopped: watch::Receiver<bool>,
    check_file: Arc<impl Fn(&QueueCheck) -> bool + Send + Sync + 'static>,
) {
    let mut tasks = JoinSet::new();
    let mut active: Vec<(tokio::task::Id, QueueCheck)> = Vec::new();
    let mut failed = Vec::new();
    let mut tick = tokio::time::interval(CHECK_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            biased;
            _ = stopped.changed() => break,
            _ = tick.tick() => {
                failed.clear();
            }
            Some(result) = tasks.join_next_with_id(), if !tasks.is_empty() => {
                let id = match &result {
                    Ok((id, _)) => *id,
                    Err(error) => error.id(),
                };
                if let Some(index) = active.iter().position(|(task, _)| *task == id) {
                    let (_, check) = active.swap_remove(index);
                    if *stopped.borrow() { break; }
                    match result {
                        Ok((_, available)) => {
                            match state.services().queue().apply_check(check.clone(), available).await {
                                Ok(assignment) => state.publish_playback(assignment.revision),
                                Err(error) => {
                                    eprintln!("Queue check result failed: {error:#}");
                                    failed.push(check);
                                }
                            }
                        }
                        Err(error) => {
                            eprintln!("Queue check task failed: {error}");
                            failed.push(check);
                        }
                    }
                }
            }
            () = state.queue_check_wakeup.notified() => {}
        }
        if *stopped.borrow() {
            break;
        }
        // Also recover post-commit notifications lost during library changes/cancellation.
        match state.services().queue().get().await {
            Ok(queue) => state.publish_playback(queue.revision),
            Err(error) => eprintln!("Queue check revision read failed: {error:#}"),
        }
        if tasks.len() < MAX_CHECKS {
            match state.services().queue().pending_checks().await {
                Ok(checks) => {
                    for check in checks {
                        if *stopped.borrow() || tasks.len() >= MAX_CHECKS {
                            break;
                        }
                        if failed.contains(&check)
                            || active.iter().any(|(_, current)| *current == check)
                        {
                            continue;
                        }
                        let operation = Arc::clone(&check_file);
                        let input = check.clone();
                        let task = tasks.spawn_blocking(move || operation(&input));
                        active.push((task.id(), check));
                    }
                }
                Err(error) => eprintln!("Queue check selection failed: {error:#}"),
            }
        }
    }
    // Discard every late result and await all file operations before acknowledging shutdown.
    while let Some(result) = tasks.join_next().await {
        if let Err(error) = result {
            eprintln!("Queue check task failed during shutdown: {error}");
        }
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests;
