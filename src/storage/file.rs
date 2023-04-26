use std::path::PathBuf;

use anyhow::bail;
use bytes::{Bytes, BytesMut};
use futures_util::{Stream, TryStreamExt};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};

use crate::error::AppError;
use crate::AppResult;

use super::Storage;

#[derive(Clone)]
pub struct FileStorage {
    dir: PathBuf,
}

impl FileStorage {
    pub async fn new(dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let dir: PathBuf = dir.into();

        if !dir.exists() {
            bail!("directory does not exist")
        }

        if !dir.is_dir() {
            bail!("not a directory");
        }

        Ok(FileStorage { dir })
    }
}

impl Storage for FileStorage {
    async fn get_object(
        &mut self,
        key: &str,
    ) -> crate::AppResult<impl Stream<Item = AppResult<Bytes>>> {
        assert!(!key.contains('/'));

        let file = fs::File::open(self.dir.join(key)).await?;
        let framed_read = FramedRead::new(file, BytesCodec::new())
            .map_ok(BytesMut::freeze)
            .map_err(Into::into);

        Ok(framed_read)
    }

    async fn put_object<S, E>(&mut self, key: &str, mut data: S) -> crate::AppResult<usize>
    where
        S: Stream<Item = Result<Bytes, E>> + Unpin,
        E: Into<AppError>,
    {
        assert!(!key.contains('/'));

        let file = fs::File::create(self.dir.join(key)).await?;
        let mut writer = FramedWrite::new(file, BytesCodec::new());

        let mut size = 0;
        while let Some(chunk) = data.try_next().await.map_err(Into::into)? {
            writer.get_mut().write_all(&chunk).await?;
            size += chunk.len();
        }

        Ok(size)
    }

    async fn delete_object(&mut self, key: &str) -> crate::AppResult<()> {
        assert!(!key.contains('/'));

        fs::remove_file(self.dir.join(key)).await?;

        Ok(())
    }
}
