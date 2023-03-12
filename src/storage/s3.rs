use aws_config::retry::RetryConfig;
use aws_sdk_s3 as s3;

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
    async fn get_object(&mut self, key: &str) -> crate::ApiResult<axum::body::Bytes> {
        let object = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        // TODO handle this error
        let bytes = object.body.collect().await.unwrap().into_bytes();
        Ok(bytes)
    }

    async fn put_object(&mut self, key: &str, data: axum::body::Bytes) -> crate::ApiResult<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(data.into())
            .send()
            .await?;
        Ok(())
    }

    async fn delete_object(&mut self, key: &str) -> crate::ApiResult<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        Ok(())
    }
}
