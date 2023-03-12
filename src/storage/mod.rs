use axum::body::Bytes;

#[cfg(feature = "s3")]
pub mod s3;

pub trait Storage {
    /// Get an object by key.
    async fn get_object(&mut self, key: &str) -> crate::ApiResult<Bytes>;

    /// Put an object's data by key.
    async fn put_object(&mut self, key: &str, data: Bytes) -> crate::ApiResult<()>;

    /// Delete an object by key.
    async fn delete_object(&mut self, key: &str) -> crate::ApiResult<()>;
}

#[derive(Clone)]
pub enum AnyStorage {
    #[cfg(feature = "s3")]
    S3(s3::S3Storage),
}

impl Storage for AnyStorage {
    async fn get_object(&mut self, key: &str) -> crate::ApiResult<Bytes> {
        match self {
            #[cfg(feature = "s3")]
            AnyStorage::S3(s3) => s3.get_object(key).await,
        }
    }

    async fn put_object(&mut self, key: &str, data: Bytes) -> crate::ApiResult<()> {
        match self {
            #[cfg(feature = "s3")]
            AnyStorage::S3(s3) => s3.put_object(key, data).await,
        }
    }

    async fn delete_object(&mut self, key: &str) -> crate::ApiResult<()> {
        match self {
            #[cfg(feature = "s3")]
            AnyStorage::S3(s3) => s3.delete_object(key).await,
        }
    }
}

#[cfg(feature = "s3")]
impl From<s3::S3Storage> for AnyStorage {
    fn from(value: s3::S3Storage) -> Self {
        AnyStorage::S3(value)
    }
}
