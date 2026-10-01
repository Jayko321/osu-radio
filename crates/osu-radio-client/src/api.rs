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
    /// Calls the sink incrementally. Dropping this future closes the response and cancels discovery.
    pub async fn discover_osu_folders(
        &self,
        request: &crate::models::DiscoverFolders,
        mut emit: impl FnMut(crate::models::FolderDiscoveryEvent) + Send,
    ) -> Result<(), ApiError> {
        let path = "/api/user-data/osu-folders/discover";
        let mut response = self
            .http
            .post(self.url(path))
            .timeout(std::time::Duration::from_secs(3600))
            .json(request)
            .send()
            .await
            .map_err(ApiError::Transport)?;
        if !response.status().is_success() {
            return Err(Self::failure(path, response).await);
        }
        let mut buffer = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(ApiError::Transport)? {
            for part in chunk.split_inclusive(|byte| *byte == b'\n') {
                buffer.extend_from_slice(part);
                if buffer.len() > 1024 * 1024 {
                    return Err(ApiError::Protocol("Discovery event exceeds 1 MiB.".into()));
                }
                if buffer.last() == Some(&b'\n') {
                    if buffer.iter().any(|byte| !byte.is_ascii_whitespace()) {
                        let event =
                            serde_json::from_slice(&buffer).map_err(ApiError::StreamDecode)?;
                        let complete =
                            matches!(event, crate::models::FolderDiscoveryEvent::Complete);
                        emit(event);
                        if complete {
                            return Ok(());
                        }
                    }
                    buffer.clear();
                }
            }
        }
        Err(ApiError::Protocol(
            "Discovery ended without a completion event.".into(),
        ))
    }
    pub async fn osu_folder_metadata(
        &self,
        marker_path: &str,
    ) -> Result<crate::models::FolderMetadata, ApiError> {
        self.folder_post("metadata", marker_path).await
    }
    pub async fn import_osu_folder(&self, marker_path: &str) -> Result<OsuFolder, ApiError> {
        self.folder_post("import", marker_path).await
    }
    async fn folder_post<T: serde::de::DeserializeOwned>(
        &self,
        operation: &str,
        marker_path: &str,
    ) -> Result<T, ApiError> {
        let path = format!("/api/user-data/osu-folders/{operation}");
        let response = self
            .http
            .post(self.url(&path))
            .timeout(std::time::Duration::from_secs(3600))
            .json(&crate::models::FolderMarker {
                marker_path: marker_path.into(),
            })
            .send()
            .await
            .map_err(ApiError::Transport)?;
        if !response.status().is_success() {
            return Err(Self::failure(&path, response).await);
        }
        response.json().await.map_err(ApiError::Decode)
    }
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

    pub async fn queue(&self) -> Result<crate::models::QueueState, ApiError> {
        self.get("/api/queue").await
    }
    pub async fn playback(&self) -> Result<crate::models::PlaybackAssignment, ApiError> {
        self.get("/api/playback").await
    }
    pub async fn append_queue(
        &self,
        audio_source_ids: &[i32],
    ) -> Result<crate::models::PlaybackAssignment, ApiError> {
        self.json_response(
            "/api/queue/items",
            self.http
                .post(self.url("/api/queue/items"))
                .json(&serde_json::json!({"audio_source_ids": audio_source_ids})),
        )
        .await
    }
    pub async fn clear_queue(&self) -> Result<crate::models::PlaybackAssignment, ApiError> {
        self.json_response("/api/queue", self.http.delete(self.url("/api/queue")))
            .await
    }
    pub async fn playback_command(
        &self,
        command: &crate::models::PlaybackCommand,
    ) -> Result<crate::models::PlaybackAssignment, ApiError> {
        let path = "/api/playback/commands";
        self.json_response(path, self.http.post(self.url(path)).json(command))
            .await
    }
    async fn json_response<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        request: reqwest::RequestBuilder,
    ) -> Result<T, ApiError> {
        let response = request.send().await.map_err(ApiError::Transport)?;
        if !response.status().is_success() {
            return Err(Self::failure(path, response).await);
        }
        response.json().await.map_err(ApiError::Decode)
    }
    /// Incremental assignment stream. Blank lines are heartbeats; cancellation closes the response.
    pub async fn playback_events(
        &self,
        mut emit: impl FnMut(crate::models::PlaybackAssignment) + Send,
    ) -> Result<(), ApiError> {
        let path = "/api/playback/events";
        let mut response = self
            .http
            .get(self.url(path))
            .timeout(std::time::Duration::from_secs(3600))
            .send()
            .await
            .map_err(ApiError::Transport)?;
        if !response.status().is_success() {
            return Err(Self::failure(path, response).await);
        }
        let mut buffer = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(ApiError::Transport)? {
            for part in chunk.split_inclusive(|byte| *byte == b'\n') {
                buffer.extend_from_slice(part);
                if buffer.len() > 1024 * 1024 {
                    return Err(ApiError::Protocol("Playback event exceeds 1 MiB.".into()));
                }
                if buffer.last() == Some(&b'\n') {
                    if buffer.iter().any(|byte| !byte.is_ascii_whitespace()) {
                        emit(serde_json::from_slice(&buffer).map_err(ApiError::StreamDecode)?);
                    }
                    buffer.clear();
                }
            }
        }
        Err(ApiError::Protocol(
            "Playback stream ended; reconnecting.".into(),
        ))
    }

    pub async fn beatmap_sets(&self) -> Result<Vec<BeatmapSet>, ApiError> {
        self.get("/api/beatmap-sets").await
    }

    pub async fn playlists(&self) -> Result<Vec<crate::models::PlaylistSummary>, ApiError> {
        self.get("/api/playlists").await
    }
    pub async fn playlist(&self, id: i32) -> Result<crate::models::Playlist, ApiError> {
        self.get(&format!("/api/playlists/{id}")).await
    }
    pub async fn create_playlist(
        &self,
        name: &str,
    ) -> Result<crate::models::PlaylistSummary, ApiError> {
        let path = "/api/playlists";
        self.json_response(
            path,
            self.http
                .post(self.url(path))
                .json(&serde_json::json!({"name":name})),
        )
        .await
    }
    pub async fn rename_playlist(
        &self,
        id: i32,
        name: &str,
    ) -> Result<crate::models::PlaylistSummary, ApiError> {
        let path = format!("/api/playlists/{id}");
        self.json_response(
            &path,
            self.http
                .patch(self.url(&path))
                .json(&serde_json::json!({"name":name})),
        )
        .await
    }
    pub async fn delete_playlist(&self, id: i32) -> Result<(), ApiError> {
        self.delete(&format!("/api/playlists/{id}")).await
    }
    pub async fn add_playlist_items(
        &self,
        id: i32,
        beatmap_ids: &[i32],
    ) -> Result<crate::models::Playlist, ApiError> {
        let path = format!("/api/playlists/{id}/items");
        self.json_response(
            &path,
            self.http
                .post(self.url(&path))
                .json(&serde_json::json!({"beatmap_ids":beatmap_ids})),
        )
        .await
    }
    pub async fn remove_playlist_item(&self, id: i32, item_id: i32) -> Result<(), ApiError> {
        self.delete(&format!("/api/playlists/{id}/items/{item_id}"))
            .await
    }
    pub async fn play_playlist(
        &self,
        id: i32,
        start_item_id: Option<i32>,
    ) -> Result<crate::models::PlaybackAssignment, ApiError> {
        let path = format!("/api/playlists/{id}/play");
        self.json_response(
            &path,
            self.http
                .post(self.url(&path))
                .json(&serde_json::json!({"start_item_id":start_item_id})),
        )
        .await
    }
    async fn delete(&self, path: &str) -> Result<(), ApiError> {
        let response = self
            .http
            .delete(self.url(path))
            .send()
            .await
            .map_err(ApiError::Transport)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(Self::failure(path, response).await)
        }
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

#[cfg(test)]
#[path = "api/folder_tests.rs"]
mod folder_tests;

#[cfg(test)]
#[path = "api/playback_tests.rs"]
mod playback_tests;

#[derive(Debug, serde::Deserialize)]
struct ErrorBody {
    #[serde(rename = "error", alias = "message")]
    message: String,
}

#[derive(Debug)]
pub enum ApiError {
    StreamDecode(serde_json::Error),
    Protocol(String),
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
            Self::StreamDecode(error) => write!(formatter, "Invalid stream event: {error}"),
            Self::Protocol(message) => formatter.write_str(message),
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
            Self::StreamDecode(error) => Some(error),
            Self::Transport(error) | Self::Decode(error) => Some(error),
            Self::File(error) => Some(error),
            Self::Protocol(_) | Self::Status { .. } | Self::AudioTooLarge => None,
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
