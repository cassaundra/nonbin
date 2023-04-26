use axum::extract::multipart::MultipartError;
use axum::http::{self, StatusCode};
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[cfg(feature = "s3")]
use aws_sdk_s3 as s3;
#[cfg(feature = "s3")]
use s3::types::SdkError;

pub type AppResult<T> = std::result::Result<T, AppError>;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum AppError {
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
    #[error("S3 error")]
    #[cfg(feature = "s3")]
    S3 {
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::InsufficientStorage => StatusCode::INSUFFICIENT_STORAGE,
            AppError::MissingFile => StatusCode::BAD_REQUEST,
            AppError::MissingFileName => StatusCode::BAD_REQUEST,
            AppError::MissingFileContentType => StatusCode::BAD_REQUEST,
            AppError::MissingDeleteKey => StatusCode::BAD_REQUEST,
            AppError::WrongDeleteKey => StatusCode::UNAUTHORIZED,
            AppError::Multipart { .. } => StatusCode::BAD_REQUEST,
            AppError::Http { .. } => StatusCode::BAD_REQUEST,
            AppError::Database { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::IO { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            #[cfg(feature = "s3")]
            AppError::S3 { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self}")).into_response()
    }
}

#[cfg(feature = "s3")]
impl From<SdkError<s3::error::DeleteObjectError>> for AppError {
    fn from(source: SdkError<s3::error::DeleteObjectError>) -> Self {
        AppError::S3 {
            source: Box::new(source),
        }
    }
}

#[cfg(feature = "s3")]
impl From<SdkError<s3::error::GetObjectError>> for AppError {
    fn from(source: SdkError<s3::error::GetObjectError>) -> Self {
        let error = source.into_service_error();
        match error.kind {
            s3::error::GetObjectErrorKind::NoSuchKey(_) => AppError::NotFound,
            _ => AppError::S3 {
                source: Box::new(error),
            },
        }
    }
}

#[cfg(feature = "s3")]
impl From<SdkError<s3::error::HeadObjectError>> for AppError {
    fn from(source: SdkError<s3::error::HeadObjectError>) -> Self {
        let error = source.into_service_error();
        match error.kind {
            s3::error::HeadObjectErrorKind::NotFound(_) => AppError::NotFound,
            _ => AppError::S3 {
                source: Box::new(error),
            },
        }
    }
}

#[cfg(feature = "s3")]
impl From<SdkError<s3::error::PutObjectError>> for AppError {
    fn from(source: SdkError<s3::error::PutObjectError>) -> Self {
        AppError::S3 {
            source: Box::new(source),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(source: sqlx::Error) -> Self {
        match source {
            sqlx::Error::RowNotFound => AppError::NotFound,
            _ => AppError::Database { source },
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(source: std::io::Error) -> Self {
        match source.kind() {
            std::io::ErrorKind::NotFound => AppError::NotFound,
            std::io::ErrorKind::StorageFull => AppError::InsufficientStorage,
            _ => AppError::IO { source },
        }
    }
}
