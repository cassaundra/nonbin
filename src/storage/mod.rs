use axum::body::Bytes;
use futures_util::{Stream, StreamExt};

use crate::error::AppError;
use crate::AppResult;

pub mod file;

#[cfg(feature = "s3")]
pub mod s3;

pub trait Storage {
    /// Get an object by key.
    async fn get_object(
        &mut self,
        key: &str,
    ) -> crate::AppResult<impl Stream<Item = AppResult<Bytes>>>;

    /// Put an object's data by key.
    async fn put_object<S, E>(&mut self, key: &str, data: S) -> crate::AppResult<usize>
    where
        S: Stream<Item = Result<Bytes, E>> + Unpin,
        E: Into<AppError>;

    /// Delete an object by key.
    async fn delete_object(&mut self, key: &str) -> crate::AppResult<()>;
}

#[derive(Clone)]
pub enum AnyStorage {
    File(file::FileStorage),
    #[cfg(feature = "s3")]
    S3(s3::S3Storage),
}

impl Storage for AnyStorage {
    async fn get_object(
        &mut self,
        key: &str,
    ) -> crate::AppResult<impl Stream<Item = AppResult<Bytes>>> {
        match self {
            AnyStorage::File(fs) => fs.get_object(key).await.map(StreamExt::boxed),
            #[cfg(feature = "s3")]
            AnyStorage::S3(s3) => s3.get_object(key).await.map(StreamExt::boxed),
        }
    }

    async fn put_object<S, E>(&mut self, key: &str, data: S) -> crate::AppResult<usize>
    where
        S: Stream<Item = Result<Bytes, E>> + Unpin,
        E: Into<AppError>,
    {
        match self {
            AnyStorage::File(fs) => fs.put_object(key, data).await,
            #[cfg(feature = "s3")]
            AnyStorage::S3(s3) => s3.put_object(key, data).await,
        }
    }

    async fn delete_object(&mut self, key: &str) -> crate::AppResult<()> {
        match self {
            AnyStorage::File(fs) => fs.delete_object(key).await,
            #[cfg(feature = "s3")]
            AnyStorage::S3(s3) => s3.delete_object(key).await,
        }
    }
}

impl From<file::FileStorage> for AnyStorage {
    fn from(value: file::FileStorage) -> Self {
        AnyStorage::File(value)
    }
}

#[cfg(feature = "s3")]
impl From<s3::S3Storage> for AnyStorage {
    fn from(value: s3::S3Storage) -> Self {
        AnyStorage::S3(value)
    }
}
