use std::borrow::Cow;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

const INTERNAL_ERROR_MESSAGE: &str = "The server failed to handle the request.";

#[derive(Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    message: Cow<'static, str>,
    source: Option<anyhow::Error>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "docs", derive(utoipa::ToSchema))]
pub(crate) struct ApiErrorBody {
    #[cfg_attr(feature = "docs", schema(value_type = String))]
    error: Cow<'static, str>,
}

impl ApiError {
    pub(crate) fn bad_request(message: impl Into<Cow<'static, str>>) -> Self {
        Self::client(StatusCode::BAD_REQUEST, message)
    }

    pub(crate) fn not_found(message: impl Into<Cow<'static, str>>) -> Self {
        Self::client(StatusCode::NOT_FOUND, message)
    }

    #[cfg(all(test, feature = "sqlite"))]
    pub(crate) const fn status(&self) -> StatusCode {
        self.status
    }

    fn client(status: StatusCode, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            status,
            message: message.into(),
            source: None,
        }
    }
}

impl<E> From<E> for ApiError
where
    E: Into<anyhow::Error>,
{
    fn from(error: E) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: Cow::Borrowed(INTERNAL_ERROR_MESSAGE),
            source: Some(error.into()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let Some(source) = self.source.as_ref() {
            eprintln!("{source:#}");
        }

        (
            self.status,
            Json(ApiErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn intentional_errors_keep_their_message_and_internal_causes_stay_out_of_json() {
        for (error, status, message) in [
            (
                ApiError::bad_request("invalid fixture input"),
                StatusCode::BAD_REQUEST,
                "invalid fixture input",
            ),
            (
                ApiError::not_found(String::from("曲 was not found")),
                StatusCode::NOT_FOUND,
                "曲 was not found",
            ),
            (
                ApiError::from(
                    anyhow::anyhow!("private fixture path").context("private storage failure"),
                ),
                StatusCode::INTERNAL_SERVER_ERROR,
                "The server failed to handle the request.",
            ),
        ] {
            let response = error.into_response();
            assert_eq!(response.status(), status);
            assert_eq!(
                response.headers()[axum::http::header::CONTENT_TYPE],
                "application/json"
            );
            let body = axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
                serde_json::json!({"error": message})
            );
        }
    }
}
