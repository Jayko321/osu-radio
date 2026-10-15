use std::{error::Error, fmt};

use crate::{
    api::{ApiClient, ApiError},
    server::{EmbeddedServer, ServerError, ServerOptions},
};

/// An embedded server together with a client pointed at the address it bound.
#[derive(Debug)]
pub struct Session {
    server: EmbeddedServer,
    api: ApiClient,
}

impl Session {
    pub async fn start(options: ServerOptions) -> Result<Self, StartError> {
        let server = EmbeddedServer::start(options)
            .await
            .map_err(StartError::Server)?;
        Self::with_server(server).await
    }

    pub(crate) async fn start_cancellable(
        options: ServerOptions,
        cancelled: tokio::sync::watch::Receiver<bool>,
    ) -> Result<Self, StartError> {
        let server = EmbeddedServer::start_cancellable(options, cancelled)
            .await
            .map_err(StartError::Server)?;
        Self::with_server(server).await
    }

    async fn with_server(server: EmbeddedServer) -> Result<Self, StartError> {
        match ApiClient::new(server.base_url()) {
            Ok(api) => Ok(Self { server, api }),
            Err(error) => {
                let _ = server.shutdown().await;
                Err(StartError::Client(error))
            }
        }
    }

    #[must_use]
    pub const fn api(&self) -> &ApiClient {
        &self.api
    }

    pub fn base_url(&self) -> &str {
        self.server.base_url()
    }

    pub async fn shutdown(&self) -> Result<(), ServerError> {
        self.server.shutdown().await
    }
}

#[derive(Debug)]
pub enum StartError {
    Server(ServerError),
    Client(ApiError),
}

impl fmt::Display for StartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Server(error) => {
                write!(formatter, "the embedded server failed to start: {error}")
            }
            Self::Client(error) => write!(formatter, "the api client could not be built: {error}"),
        }
    }
}

impl Error for StartError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Server(error) => Some(error),
            Self::Client(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn session_start_wraps_server_failure_and_client_errors_keep_their_source() {
        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("missing");
        let error = Session::start(ServerOptions {
            binary: Some(binary.clone()),
            ..Default::default()
        })
        .await
        .unwrap_err();
        assert!(
            matches!(error, StartError::Server(ServerError::BinaryNotFound(ref actual)) if actual == &binary)
        );
        assert!(
            error
                .to_string()
                .starts_with("the embedded server failed to start: ")
        );
        assert_eq!(
            error.source().unwrap().to_string(),
            ServerError::BinaryNotFound(binary).to_string()
        );
        let error = StartError::Client(ApiError::Protocol("fixture client error".into()));
        assert_eq!(
            error.to_string(),
            "the api client could not be built: fixture client error"
        );
        assert_eq!(error.source().unwrap().to_string(), "fixture client error");
    }
}
