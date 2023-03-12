use std::path::PathBuf;

use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

use super::Storage;

#[derive(Clone)]
pub struct FileStorage {
    dir: PathBuf,
}

impl FileStorage {
    pub async fn new(dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let dir: PathBuf = dir.into();

        fs::create_dir_all(&dir).await?;

        Ok(FileStorage { dir })
    }
}

impl Storage for FileStorage {
    async fn get_object(&mut self, key: &str) -> crate::ApiResult<axum::body::Bytes> {
        assert!(!key.contains('/'));

        let mut buf = Vec::with_capacity(1024);
        let mut file = BufReader::new(fs::File::open(self.dir.join(key)).await?);
        file.read_to_end(&mut buf).await?;

        Ok(buf.into())
    }

    async fn put_object(&mut self, key: &str, data: axum::body::Bytes) -> crate::ApiResult<()> {
        assert!(!key.contains('/'));

        let mut file = fs::File::create(self.dir.join(key)).await?;
        file.write_all(&data[..]).await?;

        Ok(())
    }

    async fn delete_object(&mut self, key: &str) -> crate::ApiResult<()> {
        assert!(!key.contains('/'));

        fs::remove_file(self.dir.join(key)).await?;

        Ok(())
    }
}
