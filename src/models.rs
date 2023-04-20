use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(FromRow, Serialize)]
pub struct Paste {
    pub key: String,
    pub delete_key: Option<String>,
    pub file_name: String,
    pub timestamp: DateTime<Utc>,
}
