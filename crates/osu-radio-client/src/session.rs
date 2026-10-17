use std::{error::Error, fmt};

use crate::{
    api::{ApiClient, ApiError},
    server::{EmbeddedServer, ServerError, ServerOptions},
};

/// A client session, optionally owning the embedded server it started.
#[derive(Debug)]
pub struct Session {
    server: Option<EmbeddedServer>,
    api: ApiClient,
}

impl Session {
    pub async fn start(options: ServerOptions) -> Result<Self, StartError> {
        let (_cancel, receiver) = tokio::sync::watch::channel(false);
        Self::start_cancellable(options, receiver).await
    }

    pub(crate) async fn start_cancellable(
        options: ServerOptions,
        mut cancelled: tokio::sync::watch::Receiver<bool>,
    ) -> Result<Self, StartError> {
        if let Some(url) = &options.remote_url {
            return tokio::select! {
                biased;
                _ = cancelled.wait_for(|value| *value) => Err(StartError::Server(ServerError::Cancelled)),
                result = tokio::time::timeout(options.startup_timeout, Self::connect(url)) => {
                    result.unwrap_or_else(|_| Err(StartError::Client(ApiError::Protocol(
                        "Connecting to the server timed out.".into(),
                    ))))
                }
            };
        }
        let server = EmbeddedServer::start_cancellable(options, cancelled)
            .await
            .map_err(StartError::Server)?;
        Self::with_server(server).await
    }

    async fn connect(base_url: &str) -> Result<Self, StartError> {
        let invalid_url = || {
            StartError::Client(ApiError::Protocol(
                "Server URL must be an HTTP(S) address without credentials, query or fragment."
                    .into(),
            ))
        };
        let url = reqwest::Url::parse(base_url).map_err(|_| invalid_url())?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.has_host()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(invalid_url());
        }
        let api = ApiClient::new(url.as_str()).map_err(StartError::Client)?;
        // Confirm the API is reachable before publishing Connected or enabling playback.
        api.queue().await.map_err(StartError::Client)?;
        Ok(Self { server: None, api })
    }

    async fn with_server(server: EmbeddedServer) -> Result<Self, StartError> {
        match ApiClient::new(server.base_url()) {
            Ok(api) => Ok(Self {
                server: Some(server),
                api,
            }),
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
        self.api.base_url()
    }

    pub async fn shutdown(&self) -> Result<(), ServerError> {
        if let Some(server) = &self.server {
            server.shutdown().await?;
        }
        Ok(())
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
            Self::Client(error) => write!(formatter, "the server connection failed: {error}"),
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
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    const QUEUE: &str = r#"{"entries":[],"current_entry_id":null,"history_len":0,"mode":"stopped","waiting_for_check":false,"revision":0,"playback_token":0}"#;

    fn remote_options(listener: &tokio::net::TcpListener) -> ServerOptions {
        ServerOptions {
            remote_url: Some(format!("http://{}///", listener.local_addr().unwrap())),
            binary: Some(std::path::PathBuf::from("/no/embedded/server/allowed")),
            ..Default::default()
        }
    }

    async fn respond(listener: &tokio::net::TcpListener, status: &str, body: &str) {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            request.push(stream.read_u8().await.unwrap());
        }
        assert!(request.starts_with(b"GET /api/queue HTTP/1.1"));
        stream
            .write_all(
                format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len(),
                )
                .as_bytes(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn remote_session_uses_http_without_a_child_and_leaves_server_running() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let options = remote_options(&listener);
        let server = tokio::spawn(async move {
            for _ in 0..3 {
                respond(&listener, "200 OK", QUEUE).await;
            }
        });
        let session = Session::start(options).await.unwrap();
        assert!(session.server.is_none());
        assert_eq!(session.api().base_url(), session.base_url());
        assert!(!session.base_url().ends_with('/'));
        session.shutdown().await.unwrap();
        session.shutdown().await.unwrap();
        assert!(session.api().queue().await.unwrap().entries.is_empty());
        let api = session.api().clone();
        drop(session);
        assert!(api.queue().await.unwrap().entries.is_empty());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn remote_session_rejects_invalid_urls_before_connecting() {
        for url in [
            "",
            "localhost:3000",
            "ftp://host",
            "file:///tmp/db",
            "http://host:invalid",
            "http://host?x=1",
            "http://host/#fragment",
            "http://user:password@host",
        ] {
            let error = Session::start(ServerOptions {
                remote_url: Some(url.into()),
                ..Default::default()
            })
            .await
            .unwrap_err();
            assert!(
                matches!(error, StartError::Client(ApiError::Protocol(_))),
                "{url}: {error}"
            );
        }
    }

    #[tokio::test]
    async fn remote_session_checks_status_and_json_and_can_retry() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let options = remote_options(&listener);
        let server = tokio::spawn(async move {
            respond(
                &listener,
                "503 Service Unavailable",
                r#"{"error":"unavailable"}"#,
            )
            .await;
            respond(&listener, "200 OK", "{}").await;
            respond(&listener, "200 OK", QUEUE).await;
        });
        assert!(matches!(
            Session::start(options.clone()).await,
            Err(StartError::Client(ApiError::Status { .. }))
        ));
        assert!(matches!(
            Session::start(options.clone()).await,
            Err(StartError::Client(ApiError::Decode(_)))
        ));
        Session::start(options)
            .await
            .unwrap()
            .shutdown()
            .await
            .unwrap();
        server.await.unwrap();
    }

    #[tokio::test]
    async fn remote_startup_honors_timeout_and_cancellation() {
        for cancel_start in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let options = ServerOptions {
                startup_timeout: std::time::Duration::from_millis(100),
                ..remote_options(&listener)
            };
            let (cancel, receiver) = tokio::sync::watch::channel(false);
            let connecting = tokio::spawn(Session::start_cancellable(options, receiver));
            let (mut stream, _) = listener.accept().await.unwrap();
            if cancel_start {
                cancel.send(true).unwrap();
            }
            let result = tokio::time::timeout(std::time::Duration::from_secs(2), connecting)
                .await
                .unwrap()
                .unwrap();
            if cancel_start {
                assert!(matches!(
                    result,
                    Err(StartError::Server(ServerError::Cancelled))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(StartError::Client(ApiError::Protocol(_)))
                ));
            }
            tokio::time::timeout(
                std::time::Duration::from_secs(2),
                stream.read_to_end(&mut Vec::new()),
            )
            .await
            .unwrap()
            .unwrap();
        }
    }

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
            "the server connection failed: fixture client error"
        );
        assert_eq!(error.source().unwrap().to_string(), "fixture client error");
    }
}
