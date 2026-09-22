use std::{error::Error, fmt, io::Write};

use reqwest::{Client, StatusCode};

use crate::models::{
    BeatmapSet, OsuFolder, OsuFolderChanges, RegisterOsuFolder, RegisteredFolder, UserData,
};

/// The HTTP surface of `osu-radio-server`, with no assumption about which UI calls it.
#[derive(Debug, Clone)]
pub struct ApiClient {
    base_url: String,
    http: Client,
}

impl ApiClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, ApiError> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(ApiError::Transport)?;

        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http,
        })
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn beatmap_sets(&self) -> Result<Vec<BeatmapSet>, ApiError> {
        self.get("/api/beatmap-sets").await
    }

    pub async fn search_beatmap_sets(&self, query: &str) -> Result<Vec<BeatmapSet>, ApiError> {
        let path = "/api/beatmap-sets";
        let response = self
            .http
            .get(self.url(path))
            .query(&[("q", query)])
            .send()
            .await
            .map_err(ApiError::Transport)?;
        if !response.status().is_success() {
            return Err(Self::failure(path, response).await);
        }
        response.json().await.map_err(ApiError::Decode)
    }

    pub async fn search_tracks(
        &self,
        query: &str,
    ) -> Result<Vec<crate::models::LibraryTrack>, ApiError> {
        let path = "/api/tracks";
        let response = self
            .http
            .get(self.url(path))
            .query(&[("q", query)])
            .send()
            .await
            .map_err(ApiError::Transport)?;
        if !response.status().is_success() {
            return Err(Self::failure(path, response).await);
        }
        response.json().await.map_err(ApiError::Decode)
    }

    pub async fn audio_duration(&self, id: i32) -> Result<crate::models::AudioDuration, ApiError> {
        self.get(&format!("/api/audio-sources/{id}/duration")).await
    }

    /// Download one source without retaining the response in memory. The owned file is removed
    /// when dropped, including when the future is cancelled between chunks.
    pub async fn download_audio(&self, id: i32) -> Result<tempfile::NamedTempFile, ApiError> {
        self.download_audio_bounded(id, 256 * 1024 * 1024, None)
            .await
    }

    async fn download_audio_bounded(
        &self,
        id: i32,
        limit: u64,
        directory: Option<&std::path::Path>,
    ) -> Result<tempfile::NamedTempFile, ApiError> {
        let path = format!("/api/audio-sources/{id}/audio");
        let mut response = self
            .http
            .get(self.url(&path))
            .send()
            .await
            .map_err(ApiError::Transport)?;
        if !response.status().is_success() {
            return Err(Self::failure(&path, response).await);
        }
        if response.content_length().is_some_and(|size| size > limit) {
            return Err(ApiError::AudioTooLarge);
        }
        let mut file = directory
            .map_or_else(
                tempfile::NamedTempFile::new,
                tempfile::NamedTempFile::new_in,
            )
            .map_err(ApiError::File)?;
        let mut received = 0_u64;
        while let Some(chunk) = response.chunk().await.map_err(ApiError::Transport)? {
            received = received.saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
            if received > limit {
                return Err(ApiError::AudioTooLarge);
            }
            // No detached Tokio file operation may outlive this future: dropping a cancelled
            // download must close its only file handle before unlinking it, including on Windows.
            file.write_all(&chunk).map_err(ApiError::File)?;
        }
        file.flush().map_err(ApiError::File)?;
        Ok(file)
    }

    pub async fn cover(&self, id: i32) -> Result<Option<Vec<u8>>, ApiError> {
        const LIMIT: usize = 16 * 1024 * 1024;
        let path = format!("/api/beatmaps/{id}/cover");
        let mut response = self
            .http
            .get(self.url(&path))
            .send()
            .await
            .map_err(ApiError::Transport)?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(Self::failure(&path, response).await);
        }
        if response
            .content_length()
            .is_some_and(|size| size > 16 * 1024 * 1024)
        {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(ApiError::Transport)? {
            if bytes.len().saturating_add(chunk.len()) > LIMIT {
                return Ok(None);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(Some(bytes))
    }

    pub async fn user_data(&self) -> Result<UserData, ApiError> {
        self.get("/api/user-data").await
    }

    pub async fn osu_folders(&self) -> Result<Vec<OsuFolder>, ApiError> {
        self.get("/api/user-data/osu-folders").await
    }

    pub async fn register_osu_folder(
        &self,
        folder: &RegisterOsuFolder,
    ) -> Result<RegisteredFolder, ApiError> {
        let path = "/api/user-data/osu-folders";
        let response = self
            .http
            .post(self.url(path))
            .json(folder)
            .send()
            .await
            .map_err(ApiError::Transport)?;

        let status = response.status();
        if status != StatusCode::CREATED && status != StatusCode::CONFLICT {
            return Err(Self::failure(path, response).await);
        }

        let registered = response
            .json::<OsuFolder>()
            .await
            .map_err(ApiError::Decode)?;

        if status == StatusCode::CREATED {
            Ok(RegisteredFolder::Created(registered))
        } else {
            Ok(RegisteredFolder::AlreadyRegistered(registered))
        }
    }

    pub async fn update_osu_folder(
        &self,
        id: i32,
        changes: &OsuFolderChanges,
    ) -> Result<OsuFolder, ApiError> {
        let path = format!("/api/user-data/osu-folders/{id}");
        let response = self
            .http
            .patch(self.url(&path))
            .json(changes)
            .send()
            .await
            .map_err(ApiError::Transport)?;

        if !response.status().is_success() {
            return Err(Self::failure(&path, response).await);
        }

        response.json().await.map_err(ApiError::Decode)
    }

    pub async fn remove_osu_folder(&self, id: i32) -> Result<(), ApiError> {
        let path = format!("/api/user-data/osu-folders/{id}");
        let response = self
            .http
            .delete(self.url(&path))
            .send()
            .await
            .map_err(ApiError::Transport)?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(Self::failure(&path, response).await)
        }
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let response = self
            .http
            .get(self.url(path))
            .send()
            .await
            .map_err(ApiError::Transport)?;

        if !response.status().is_success() {
            return Err(Self::failure(path, response).await);
        }

        response.json().await.map_err(ApiError::Decode)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    async fn failure(path: &str, response: reqwest::Response) -> ApiError {
        let status = response.status();
        let message = response
            .json::<ErrorBody>()
            .await
            .map_or_else(|_| status.to_string(), |body| body.message);

        ApiError::Status {
            path: path.to_owned(),
            status,
            message,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct ErrorBody {
    #[serde(rename = "error", alias = "message")]
    message: String,
}

#[derive(Debug)]
pub enum ApiError {
    File(std::io::Error),
    AudioTooLarge,
    Transport(reqwest::Error),
    Decode(reqwest::Error),
    Status {
        path: String,
        status: StatusCode,
        message: String,
    },
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(error) => write!(formatter, "could not save downloaded audio: {error}"),
            Self::AudioTooLarge => formatter.write_str("audio exceeds the 256 MiB download limit"),
            Self::Transport(error) => write!(formatter, "the server could not be reached: {error}"),
            Self::Decode(error) => {
                write!(formatter, "the server sent an unexpected response: {error}")
            }
            Self::Status {
                path,
                status,
                message,
            } => write!(formatter, "`{path}` answered {status}: {message}"),
        }
    }
}

impl Error for ApiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(error) | Self::Decode(error) => Some(error),
            Self::File(error) => Some(error),
            Self::Status { .. } | Self::AudioTooLarge => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn serve(response: Vec<u8>) -> (ApiClient, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = ApiClient::new(format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(stream.read_u8().await.unwrap());
            }
            assert!(request.starts_with(b"GET /api/audio-sources/7/audio HTTP/1.1"));
            let _ = stream.write_all(&response).await;
        });
        (api, server)
    }
    #[tokio::test]
    async fn audio_download_preserves_bytes_and_deletes_its_file_on_drop() {
        let bytes = b"\x00\xffmp3\r\ncontent";
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        )
        .into_bytes();
        response.extend_from_slice(bytes);
        let (api, server) = serve(response).await;
        let file = api.download_audio(7).await.unwrap();
        assert_eq!(std::fs::read(file.path()).unwrap(), bytes);
        let path = file.path().to_owned();
        drop(file);
        assert!(!path.exists());
        server.await.unwrap();
    }
    #[tokio::test]
    async fn audio_limit_checks_headers_and_streamed_body_and_cleans_failed_downloads() {
        let directory = tempfile::tempdir().unwrap();
        for response in [
            b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\n123456789".to_vec(),
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\n12345\r\n4\r\n6789\r\n0\r\n\r\n".to_vec(),
        ] {
            let (api, server) = serve(response).await;
            assert!(matches!(api.download_audio_bounded(7, 8, Some(directory.path())).await, Err(ApiError::AudioTooLarge)));
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
            server.await.unwrap();
        }
        let (api, server) =
            serve(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\n123".to_vec()).await;
        assert!(matches!(
            api.download_audio_bounded(7, 8, Some(directory.path()))
                .await,
            Err(ApiError::Transport(_))
        ));
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        server.await.unwrap();
    }
    #[tokio::test]
    async fn audio_failure_uses_server_error_field() {
        let body = br#"{"error":"Audio source was not found."}"#;
        let mut response = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        let (api, server) = serve(response).await;
        assert!(
            api.download_audio(7)
                .await
                .unwrap_err()
                .to_string()
                .contains("Audio source was not found.")
        );
        server.await.unwrap();
    }
    #[tokio::test]
    async fn cancelling_a_partial_download_closes_and_removes_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = ApiClient::new(format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let (release, held) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(stream.read_u8().await.unwrap());
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\npartial")
                .await
                .unwrap();
            let _ = held.await;
        });
        let path = directory.path().to_owned();
        let download =
            tokio::spawn(async move { api.download_audio_bounded(7, 100, Some(&path)).await });
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while std::fs::read_dir(directory.path()).unwrap().count() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        download.abort();
        assert!(download.await.unwrap_err().is_cancelled());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        let _ = release.send(());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn search_encodes_query_as_a_single_literal_parameter() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let api = ApiClient::new(format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(stream.read_u8().await.unwrap());
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n[]")
                .await
                .unwrap();
            String::from_utf8(request).unwrap()
        });
        assert!(api.search_tracks("ЁЖ a+b%_&?#").await.unwrap().is_empty());
        assert!(
            server
                .await
                .unwrap()
                .starts_with("GET /api/tracks?q=%D0%81%D0%96+a%2Bb%25_%26%3F%23 HTTP/1.1\r\n")
        );
    }
}
