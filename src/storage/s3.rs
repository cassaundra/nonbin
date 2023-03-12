use std::borrow::Cow;

use aws_config::retry::RetryConfig;
use aws_sdk_s3 as s3;

use super::Storage;

#[derive(Clone)]
pub struct S3Storage {
    client: s3::Client,
    bucket: String,
}

impl S3Storage {
    pub async fn new(
        region: impl Into<Cow<'static, str>>,
        bucket: impl Into<String>,
        endpoint: Option<impl Into<String>>,
    ) -> Self {
        let client = {
            let sdk_config = aws_config::load_from_env().await;
            let mut config_builder = s3::config::Builder::from(&sdk_config)
                .region(s3::Region::new(region))
                .retry_config(RetryConfig::disabled());

            if let Some(endpoint) = endpoint {
                config_builder = config_builder.endpoint_url(endpoint);
            }

            let s3_config = config_builder.build();

            s3::Client::from_conf(s3_config)
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
