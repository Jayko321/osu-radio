use radio_services::{FolderChanges, RegisterFolderError, UserDataOverview};
use std::path::PathBuf;

use axum::{
    Json,
    extract::{Path as RoutePath, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use radio_services::model::OsuInstallation;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{error::ApiError, state::AppState};

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct UserDataResponse {
    pub(crate) id: i32,
    pub(crate) osu_folders: Vec<OsuFolderResponse>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct OsuFolderResponse {
    pub(crate) id: i32,
    #[cfg_attr(feature = "docs", schema(value_type = String, example = "lazer"))]
    pub(crate) kind: &'static str,
    pub(crate) root_path: String,
    pub(crate) marker_path: String,
    pub(crate) label: Option<String>,
    pub(crate) enabled: bool,
    #[cfg_attr(feature = "docs", schema(example = "2026-01-01T00:00:00Z"))]
    pub(crate) last_scanned_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct RegisterOsuFolderRequest {
    #[cfg_attr(feature = "docs", schema(value_type = String, example = "D:/osu"))]
    pub(crate) path: PathBuf,
    #[serde(default)]
    pub(crate) label: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct UpdateOsuFolderRequest {
    /// Absent leaves the label alone; an explicit `null` clears it.
    #[allow(clippy::option_option)]
    #[serde(default, deserialize_with = "deserialize_present_field")]
    #[cfg_attr(feature = "docs", schema(value_type = Option<String>, nullable = true))]
    pub(crate) label: Option<Option<String>>,
    #[serde(default)]
    pub(crate) enabled: Option<bool>,
}

impl From<OsuInstallation> for OsuFolderResponse {
    fn from(installation: OsuInstallation) -> Self {
        Self {
            id: installation.id,
            kind: installation.kind.as_str(),
            root_path: installation.root_path.to_string_lossy().into_owned(),
            marker_path: installation.marker_path.to_string_lossy().into_owned(),
            label: installation.label,
            enabled: installation.enabled,
            last_scanned_at: installation.last_scanned_at,
        }
    }
}

impl From<UserDataOverview> for UserDataResponse {
    fn from(overview: UserDataOverview) -> Self {
        Self {
            id: overview.id,
            osu_folders: overview
                .osu_folders
                .into_iter()
                .map(OsuFolderResponse::from)
                .collect(),
        }
    }
}

fn registration_error(error: RegisterFolderError) -> ApiError {
    match error {
        RegisterFolderError::RelativePath(path) => ApiError::bad_request(format!(
            "`{}` is not an absolute path. Register an osu! folder by its full path.",
            path.display()
        )),
        RegisterFolderError::NotAnOsuFolder(path) => ApiError::bad_request(format!(
            "No osu! installation was found in `{}`. Expected a client.realm or osu!.db there.",
            path.display()
        )),
        RegisterFolderError::Ambiguous { path, found } => ApiError::bad_request(format!(
            "`{}` holds {} osu! installations. Register each one by its own folder.",
            path.display(),
            found.len()
        )),
        RegisterFolderError::Failed(error) => ApiError::from(error),
    }
}

/// Distinguishes an absent JSON field from one explicitly set to `null`, which plain
/// `Option<Option<T>>` collapses into the same value.
fn deserialize_present_field<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[cfg_attr(
    feature = "docs",
    utoipa::path(
        get,
        path = "/api/user-data",
        tag = "user-data",
        description = "The singleton settings row with every registered osu! folder.",
        responses(
            (status = OK, body = UserDataResponse),
            (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody)
        )
    )
)]
pub(crate) async fn get_user_data(
    State(state): State<AppState>,
) -> Result<Json<UserDataResponse>, ApiError> {
    let overview = state.services().user_data().overview().await?;

    Ok(Json(UserDataResponse::from(overview)))
}

#[cfg_attr(
    feature = "docs",
    utoipa::path(
        get,
        path = "/api/user-data/osu-folders",
        tag = "user-data",
        description = "Every registered osu! folder. An empty list is a valid state.",
        responses(
            (status = OK, body = Vec<OsuFolderResponse>),
            (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody)
        )
    )
)]
pub(crate) async fn list_osu_folders(
    State(state): State<AppState>,
) -> Result<Json<Vec<OsuFolderResponse>>, ApiError> {
    let folders = state.services().osu_installations().all().await?;

    Ok(Json(
        folders.into_iter().map(OsuFolderResponse::from).collect(),
    ))
}

#[cfg_attr(
    feature = "docs",
    utoipa::path(
        post,
        path = "/api/user-data/osu-folders",
        tag = "user-data",
        description = "Registers an osu! folder by its absolute path. Discovery re-proves the installation and derives its kind; the client's claim is never trusted.",
        request_body = RegisterOsuFolderRequest,
        responses(
            (status = CREATED, body = OsuFolderResponse),
            (status = CONFLICT, description = "The folder is already registered.", body = OsuFolderResponse),
            (status = BAD_REQUEST, description = "The path is relative, holds no installation, or holds more than one.", body = crate::error::ApiErrorBody),
            (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody)
        )
    )
)]
pub(crate) async fn register_osu_folder(
    State(state): State<AppState>,
    Json(request): Json<RegisterOsuFolderRequest>,
) -> Result<Response, ApiError> {
    let registered = state
        .services()
        .osu_installations()
        .register_folder(request.path, request.label)
        .await
        .map_err(registration_error)?;

    let status = if registered.was_created() {
        StatusCode::CREATED
    } else {
        StatusCode::CONFLICT
    };
    let folder = OsuFolderResponse::from(registered.into_installation());

    Ok((status, Json(folder)).into_response())
}

#[cfg_attr(
    feature = "docs",
    utoipa::path(
        patch,
        path = "/api/user-data/osu-folders/{id}",
        tag = "user-data",
        description = "Edits `label` and `enabled` only. An absent field is left alone; an explicit null label clears it. Changing the path means DELETE then POST.",
        params(("id" = i32, Path, description = "The registered folder's id.")),
        request_body = UpdateOsuFolderRequest,
        responses(
            (status = OK, body = OsuFolderResponse),
            (status = NOT_FOUND, body = crate::error::ApiErrorBody),
            (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody)
        )
    )
)]
pub(crate) async fn update_osu_folder(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<i32>,
    Json(request): Json<UpdateOsuFolderRequest>,
) -> Result<Json<OsuFolderResponse>, ApiError> {
    let updated = state
        .services()
        .osu_installations()
        .update(
            id,
            FolderChanges {
                label: request.label,
                enabled: request.enabled,
            },
        )
        .await?;

    updated.map_or_else(
        || Err(unknown_folder(id)),
        |folder| Ok(Json(OsuFolderResponse::from(folder))),
    )
}

#[cfg_attr(
    feature = "docs",
    utoipa::path(
        delete,
        path = "/api/user-data/osu-folders/{id}",
        tag = "user-data",
        description = "Removes a registered folder. Every beatmap set imported from it cascades away.",
        params(("id" = i32, Path, description = "The registered folder's id.")),
        responses(
            (status = NO_CONTENT, description = "The folder was removed."),
            (status = NOT_FOUND, body = crate::error::ApiErrorBody),
            (status = INTERNAL_SERVER_ERROR, body = crate::error::ApiErrorBody)
        )
    )
)]
pub(crate) async fn remove_osu_folder(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<i32>,
) -> Result<StatusCode, ApiError> {
    let removed = state.services().osu_installations().delete(id).await?;

    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(unknown_folder(id))
    }
}

fn unknown_folder(id: i32) -> ApiError {
    ApiError::not_found(format!("No osu! folder is registered with id {id}."))
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use crate::test_support::{empty_state, lazer_folder};

    use super::*;

    async fn register(state: &AppState, path: PathBuf, label: Option<&str>) -> Response {
        register_osu_folder(
            State(state.clone()),
            Json(RegisterOsuFolderRequest {
                path,
                label: label.map(str::to_owned),
            }),
        )
        .await
        .expect("registration should answer")
    }

    #[tokio::test]
    async fn registering_a_folder_answers_201_and_listing_returns_it() {
        let state = empty_state().await;
        let folder = lazer_folder();

        let response = register(&state, folder.path().to_path_buf(), Some("Desktop")).await;
        assert_eq!(response.status(), StatusCode::CREATED);

        let Json(listed) = list_osu_folders(State(state))
            .await
            .expect("folders should be listed");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].kind, "lazer");
        assert_eq!(listed[0].label.as_deref(), Some("Desktop"));
        assert!(listed[0].enabled);
        assert_eq!(listed[0].last_scanned_at, None);
    }

    #[tokio::test]
    async fn registering_the_same_folder_again_answers_409() {
        let state = empty_state().await;
        let folder = lazer_folder();

        register(&state, folder.path().to_path_buf(), Some("Desktop")).await;
        let response = register(&state, folder.path().to_path_buf(), Some("Changed")).await;

        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("conflict body should be readable");
        let stored: serde_json::Value = serde_json::from_slice(&body)
            .expect("conflict body should retain the folder response shape");
        assert_eq!(stored["id"], 1);
        assert_eq!(stored["label"], "Desktop");
        assert_eq!(stored["enabled"], true);
        assert!(stored["last_scanned_at"].is_null());
    }

    #[tokio::test]
    async fn registering_a_folder_without_an_installation_answers_400() {
        let state = empty_state().await;
        let empty = tempfile::TempDir::new().expect("temp dir");

        let error = register_osu_folder(
            State(state),
            Json(RegisterOsuFolderRequest {
                path: empty.path().to_path_buf(),
                label: None,
            }),
        )
        .await
        .expect_err("an empty folder should be rejected");

        assert_eq!(error.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn get_user_data_returns_the_settings_row_with_its_folders() {
        let state = empty_state().await;
        let folder = lazer_folder();
        register(&state, folder.path().to_path_buf(), None).await;

        let Json(user_data) = get_user_data(State(state))
            .await
            .expect("user data should load");

        assert_eq!(user_data.id, 1);
        assert_eq!(user_data.osu_folders.len(), 1);
    }

    #[tokio::test]
    async fn patching_only_the_supplied_fields_leaves_the_rest_alone() {
        let state = empty_state().await;
        let folder = lazer_folder();
        register(&state, folder.path().to_path_buf(), Some("Desktop")).await;

        let Json(updated) = update_osu_folder(
            State(state.clone()),
            RoutePath(1),
            Json(UpdateOsuFolderRequest {
                enabled: Some(false),
                ..UpdateOsuFolderRequest::default()
            }),
        )
        .await
        .expect("the folder should update");

        assert!(!updated.enabled);
        assert_eq!(updated.label.as_deref(), Some("Desktop"));

        let Json(cleared) = update_osu_folder(
            State(state),
            RoutePath(1),
            Json(UpdateOsuFolderRequest {
                label: Some(None),
                ..UpdateOsuFolderRequest::default()
            }),
        )
        .await
        .expect("the folder should update");

        assert_eq!(cleared.label, None);
        assert!(!cleared.enabled);
    }

    #[tokio::test]
    async fn an_absent_label_and_an_explicit_null_label_differ() {
        let absent: UpdateOsuFolderRequest =
            serde_json::from_str(r#"{"enabled":true}"#).expect("body should parse");
        let null = serde_json::from_str::<UpdateOsuFolderRequest>(r#"{"label":null}"#)
            .expect("body should parse");

        assert_eq!(absent.label, None);
        assert_eq!(null.label, Some(None));
    }

    #[tokio::test]
    async fn deleting_a_folder_answers_204_then_404() {
        let state = empty_state().await;
        let folder = lazer_folder();
        register(&state, folder.path().to_path_buf(), None).await;

        let status = remove_osu_folder(State(state.clone()), RoutePath(1))
            .await
            .expect("the folder should be removed");
        assert_eq!(status, StatusCode::NO_CONTENT);

        let error = remove_osu_folder(State(state), RoutePath(1))
            .await
            .expect_err("a removed folder should not be found");
        assert_eq!(error.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn patching_an_unknown_folder_answers_404() {
        let state = empty_state().await;

        let error = update_osu_folder(
            State(state),
            RoutePath(404),
            Json(UpdateOsuFolderRequest {
                enabled: Some(false),
                ..UpdateOsuFolderRequest::default()
            }),
        )
        .await
        .expect_err("an unknown folder should not be found");

        assert_eq!(error.status(), StatusCode::NOT_FOUND);
    }
}
