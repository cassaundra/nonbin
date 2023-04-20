use sqlx::AnyPool;

use crate::models::Paste;

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

    /// Get all pastes.
    pub async fn get_all_pastes(&mut self) -> crate::AppResult<Vec<Paste>> {
        let mut conn = self.pool.acquire().await?;
        Ok(sqlx::query_as::<_, Paste>("SELECT * FROM paste")
            .fetch_all(&mut conn)
            .await?)
    }

    /// Get a paste by key.
    pub async fn get_paste(&mut self, key: &str) -> crate::AppResult<Paste> {
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
    ) -> crate::AppResult<Paste> {
        let mut conn = self.pool.acquire().await?;
        let paste = sqlx::query_as::<_, Paste>(
            "INSERT INTO paste (key, delete_key, file_name) VALUES (?, ?, ?) RETURNING key, \
             delete_key, file_name, timestamp",
        )
        .bind(key)
        .bind(delete_key)
        .bind(file_name)
        .fetch_one(&mut conn)
        .await?;
        Ok(paste)
    }

    /// Delete a paste by key.
    pub async fn delete_paste(&mut self, key: &str) -> crate::AppResult<()> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query("DELETE FROM paste WHERE key = ?")
            .bind(key)
            .execute(&mut conn)
            .await?;
        Ok(())
    }
}
