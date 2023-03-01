use aws_sdk_s3 as s3;
use axum::extract::multipart::MultipartError;
use axum::http::{self, StatusCode};
use axum::response::{IntoResponse, Response};
use s3::types::SdkError;
use thiserror::Error;

pub type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ApiError {
    #[error("paste not found")]
    PasteNotFound,
    #[error("missing multipart file")]
    MissingFile,
    #[error("missing multipart file name")]
    MissingFileName,
    #[error("missing content type for multipart file")]
    MissingFileContentType,
    #[error("missing delete key")]
    MissingDeleteKey,
    #[error("wrong delete key")]
    WrongDeleteKey,
    #[error("error reading multipart data")]
    Multipart {
        #[from]
        source: MultipartError,
    },
    #[error("http error")]
    Http {
        #[from]
        source: http::Error,
    },
    #[error("other error")]
    Other {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            ApiError::PasteNotFound => StatusCode::NOT_FOUND,
            ApiError::MissingFile => StatusCode::BAD_REQUEST,
            ApiError::MissingFileName => StatusCode::BAD_REQUEST,
            ApiError::MissingFileContentType => StatusCode::BAD_REQUEST,
            ApiError::MissingDeleteKey => StatusCode::BAD_REQUEST,
            ApiError::WrongDeleteKey => StatusCode::UNAUTHORIZED,
            ApiError::Multipart { .. } => StatusCode::BAD_REQUEST,
            ApiError::Http { .. } => StatusCode::BAD_REQUEST,
            ApiError::Other { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self}")).into_response()
    }
}

impl From<SdkError<s3::error::DeleteObjectError>> for ApiError {
    fn from(source: SdkError<s3::error::DeleteObjectError>) -> Self {
        ApiError::Other {
            source: Box::new(source),
        }
    }
}

impl From<SdkError<s3::error::GetObjectError>> for ApiError {
    fn from(source: SdkError<s3::error::GetObjectError>) -> Self {
        let error = source.into_service_error();
        match error.kind {
            s3::error::GetObjectErrorKind::NoSuchKey(_) => ApiError::PasteNotFound,
            _ => ApiError::Other {
                source: Box::new(error),
            },
        }
    }
}

impl From<SdkError<s3::error::HeadObjectError>> for ApiError {
    fn from(source: SdkError<s3::error::HeadObjectError>) -> Self {
        let error = source.into_service_error();
        match error.kind {
            s3::error::HeadObjectErrorKind::NotFound(_) => ApiError::PasteNotFound,
            _ => ApiError::Other {
                source: Box::new(error),
            },
        }
    }
}

impl From<SdkError<s3::error::PutObjectError>> for ApiError {
    fn from(source: SdkError<s3::error::PutObjectError>) -> Self {
        ApiError::Other {
            source: Box::new(source),
        }
    }
}
