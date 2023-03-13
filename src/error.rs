use axum::extract::multipart::MultipartError;
use axum::http::{self, StatusCode};
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[cfg(feature = "s3")]
use aws_sdk_s3 as s3;
#[cfg(feature = "s3")]
use s3::types::SdkError;

pub type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ApiError {
    #[error("not found")]
    NotFound,
    #[error("insufficient storage")]
    InsufficientStorage,
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
    #[error("database error")]
    Database { source: sqlx::Error },
    #[error("IO error")]
    IO { source: std::io::Error },
    #[error("other error")]
    #[cfg(feature = "s3")]
    S3 {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::InsufficientStorage => StatusCode::INSUFFICIENT_STORAGE,
            ApiError::MissingFile => StatusCode::BAD_REQUEST,
            ApiError::MissingFileName => StatusCode::BAD_REQUEST,
            ApiError::MissingFileContentType => StatusCode::BAD_REQUEST,
            ApiError::MissingDeleteKey => StatusCode::BAD_REQUEST,
            ApiError::WrongDeleteKey => StatusCode::UNAUTHORIZED,
            ApiError::Multipart { .. } => StatusCode::BAD_REQUEST,
            ApiError::Http { .. } => StatusCode::BAD_REQUEST,
            ApiError::Database { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::IO { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            #[cfg(feature = "s3")]
            ApiError::S3 { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self}")).into_response()
    }
}

#[cfg(feature = "s3")]
impl From<SdkError<s3::error::DeleteObjectError>> for ApiError {
    fn from(source: SdkError<s3::error::DeleteObjectError>) -> Self {
        ApiError::S3 {
            source: Box::new(source),
        }
    }
}

#[cfg(feature = "s3")]
impl From<SdkError<s3::error::GetObjectError>> for ApiError {
    fn from(source: SdkError<s3::error::GetObjectError>) -> Self {
        let error = source.into_service_error();
        match error.kind {
            s3::error::GetObjectErrorKind::NoSuchKey(_) => ApiError::NotFound,
            _ => ApiError::S3 {
                source: Box::new(error),
            },
        }
    }
}

#[cfg(feature = "s3")]
impl From<SdkError<s3::error::HeadObjectError>> for ApiError {
    fn from(source: SdkError<s3::error::HeadObjectError>) -> Self {
        let error = source.into_service_error();
        match error.kind {
            s3::error::HeadObjectErrorKind::NotFound(_) => ApiError::NotFound,
            _ => ApiError::S3 {
                source: Box::new(error),
            },
        }
    }
}

#[cfg(feature = "s3")]
impl From<SdkError<s3::error::PutObjectError>> for ApiError {
    fn from(source: SdkError<s3::error::PutObjectError>) -> Self {
        ApiError::S3 {
            source: Box::new(source),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(source: sqlx::Error) -> Self {
        match source {
            sqlx::Error::RowNotFound => ApiError::NotFound,
            _ => ApiError::Database { source },
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(source: std::io::Error) -> Self {
        match source.kind() {
            std::io::ErrorKind::NotFound => ApiError::NotFound,
            std::io::ErrorKind::StorageFull => ApiError::InsufficientStorage,
            _ => ApiError::IO { source },
        }
    }
}
