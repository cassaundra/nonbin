use sqlx::AnyPool;

use crate::types::Paste;

#[derive(Clone)]
pub struct Database {
    pool: AnyPool,
}

impl Database {
    /// Connect to a database by URL.
    pub async fn connect(url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            pool: AnyPool::connect(url).await?,
        })
    }

    /// Get a paste by key.
    pub async fn get_paste(&mut self, key: &str) -> crate::ApiResult<Paste> {
        let mut conn = self.pool.acquire().await?;
        let paste = sqlx::query_as::<_, Paste>(
            "SELECT key, delete_key, timestamp, file_name FROM paste WHERE key = ?",
        )
        .bind(key)
        .fetch_one(&mut conn)
        .await?;
        Ok(paste)
    }

    /// Insert a paste.
    pub async fn insert_paste(
        &mut self,
        key: &str,
        delete_key: &str,
        file_name: &str,
    ) -> crate::ApiResult<()> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query("INSERT INTO paste (key, delete_key, file_name) VALUES (?, ?, ?)")
            .bind(key)
            .bind(delete_key)
            .bind(file_name)
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    /// Delete a paste by key.
    pub async fn delete_paste(&mut self, key: &str) -> crate::ApiResult<()> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query("DELETE FROM paste WHERE key = ?")
            .bind(key)
            .execute(&mut conn)
            .await?;
        Ok(())
    }
}
