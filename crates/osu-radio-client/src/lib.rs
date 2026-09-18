#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

pub mod api;
#[cfg(feature = "mock")]
pub mod mock;
pub mod models;
pub mod server;
pub mod session;
pub mod view_models;

pub use api::{ApiClient, ApiError};
pub use models::{
    AudioSource, BeatmapSet, OsuFolder, OsuFolderChanges, RegisterOsuFolder, RegisteredFolder,
    UserData,
};
pub use server::{EmbeddedServer, ServerError, ServerOptions};
pub use session::{Session, StartError};
pub use view_models::Track;

/// The state of one thing a UI is showing. Frontends differ in how they render it, not in the
/// states themselves, so the enum lives here rather than in any one of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Loading<T> {
    #[default]
    Idle,
    Pending,
    Ready(T),
    Failed(String),
}

impl<T> Loading<T> {
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Ready(value) => Some(value),
            _ => None,
        }
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failed(message) => Some(message),
            _ => None,
        }
    }

    /// Turns any error into the message a UI would show, so callers do not each reimplement it.
    pub fn from_result<E: std::error::Error>(result: Result<T, E>) -> Self {
        match result {
            Ok(value) => Self::Ready(value),
            Err(error) => Self::Failed(describe(&error)),
        }
    }
}

/// Flattens an error and its sources into one line, so a UI shows the cause rather than a bare
/// wrapper message.
pub fn describe(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();

    while let Some(cause) = source {
        let text = cause.to_string();

        if !message.contains(&text) {
            message.push_str(": ");
            message.push_str(&text);
        }

        source = cause.source();
    }

    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_result_becomes_a_message_a_ui_can_show() {
        let result: Result<u8, ApiError> = Err(ApiError::Status {
            path: "/api/beatmap-sets".to_owned(),
            status: reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            message: "Something went wrong.".to_owned(),
        });

        let loading = Loading::from_result(result);

        assert_eq!(
            loading.error(),
            Some("`/api/beatmap-sets` answered 500 Internal Server Error: Something went wrong.")
        );
    }

    #[test]
    fn a_ready_value_carries_no_error() {
        let loading = Loading::<u8>::from_result(Ok::<u8, ApiError>(7));

        assert_eq!(loading.value(), Some(&7));
        assert_eq!(loading.error(), None);
    }
}
