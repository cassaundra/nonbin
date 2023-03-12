use chrono::{DateTime, Utc};
use sqlx::FromRow;

pub mod api;

#[derive(FromRow)]
pub struct Paste {
    pub key: String,
    pub delete_key: Option<String>,
    pub file_name: String,
    pub timestamp: DateTime<Utc>,
}
