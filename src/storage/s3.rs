use aws_config::retry::RetryConfig;
use aws_sdk_s3 as s3;

use bytes::{Bytes, BytesMut};
use futures_util::{Stream, TryStreamExt};

use crate::error::AppError;
use crate::AppResult;

use super::Storage;

#[derive(Clone)]
pub struct S3Storage {
    client: s3::Client,
    bucket: String,
}

impl S3Storage {
    pub async fn new(bucket: &str, region: Option<&str>, endpoint: Option<&str>) -> Self {
        let client = {
            let mut config_loader = aws_config::from_env().retry_config(RetryConfig::disabled());
            if let Some(region) = region {
                config_loader = config_loader.region(s3::Region::new(region.to_owned()));
            }
            if let Some(endpoint) = endpoint {
                config_loader = config_loader.endpoint_url(endpoint);
            }
            let sdk_config = config_loader.load().await;

            s3::Client::new(&sdk_config)
        };

        S3Storage {
            client,
            bucket: bucket.into(),
        }
    }
}

impl Storage for S3Storage {
    async fn get_object(
        &mut self,
        key: &str,
    ) -> crate::AppResult<impl Stream<Item = AppResult<Bytes>>> {
        let object = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        Ok(object.body.map_err(|e| AppError::S3 {
            source: Box::new(e),
        }))
    }

    async fn put_object<S, E>(&mut self, key: &str, data: S) -> crate::AppResult<usize>
    where
        S: Stream<Item = Result<Bytes, E>> + Unpin,
        E: Into<AppError>,
    {
        // TODO figure out how to convert to ByteStream
        let body = data
            .try_collect::<BytesMut>()
            .await
            .map_err(Into::into)?
            .freeze();
        let size = body.len();
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body.into())
            .send()
            .await?;
        Ok(size)
    }

    async fn delete_object(&mut self, key: &str) -> crate::AppResult<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        Ok(())
    }
}
