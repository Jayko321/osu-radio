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
        let api = ApiClient::new(server.base_url()).map_err(StartError::Client)?;

        Ok(Self { server, api })
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
