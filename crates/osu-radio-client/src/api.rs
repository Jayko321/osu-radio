use std::{error::Error, fmt};

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

    pub async fn audio_duration(&self, id: i32) -> Result<crate::models::AudioDuration, ApiError> {
        self.get(&format!("/api/audio-sources/{id}/duration")).await
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
    message: String,
}

#[derive(Debug)]
pub enum ApiError {
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
            Self::Status { .. } => None,
        }
    }
}
