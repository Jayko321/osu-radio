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

    #[cfg(test)]
    pub(crate) fn status(&self) -> StatusCode {
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
