use axum::body::Bytes;
use chrono::{DateTime, Utc};
use futures_util::Stream;
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::Paste;
use crate::storage::Storage;
use crate::words::generate_key;
use crate::{App, AppResult};

pub async fn fetch_data(
    app: &mut App,
    key: &str,
) -> crate::AppResult<impl Stream<Item = AppResult<Bytes>>> {
    app.storage.get_object(key).await
}

pub async fn create<S, E>(app: &mut App, file_name: &str, data: S) -> crate::AppResult<Paste>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Into<AppError>,
{
    let key = generate_key(&app.word_lists);
    let delete_key = Uuid::new_v4().to_string();

    info!("new paste: key='{key}', file='{file_name}'");

    let paste = app
        .database
        .insert_paste(&key, &delete_key, file_name)
        .await?;
    let size = app.storage.put_object(&key, data).await?;

    info!("size of '{key}': {size} bytes");

    Ok(paste)
}

pub async fn delete(app: &mut App, key: &str) -> crate::AppResult<()> {
    app.database.delete_paste(key).await?;
    app.storage.delete_object(key).await?;
    Ok(())
}

pub async fn purge_expired(app: &mut App) -> crate::AppResult<()> {
    let Some(expiration_secs) = app.config.limits.expiration_secs else {
        warn!("tried to purge expired, but no expiration limit was set");
        return Ok(());
    };

    let pastes = app.database.get_all_pastes().await?;

    let now = Utc::now();

    let mut count = 0;
    for paste in pastes {
        if is_expired_inner(&paste, &now, expiration_secs) {
            app.database.delete_paste(&paste.key).await?;
            app.storage.delete_object(&paste.key).await?;
            count += 1;
        }
    }

    if count > 0 {
        info!("deleted {count} pastes");
    }

    Ok(())
}

pub async fn is_expired(app: &mut App, key: &str) -> crate::AppResult<bool> {
    let Some(expiration_secs) = app.config.limits.expiration_secs else { return Ok(false) };

    let paste = app.database.get_paste(key).await?;

    Ok(is_expired_inner(&paste, &Utc::now(), expiration_secs))
}

fn is_expired_inner(paste: &Paste, current_time: &DateTime<Utc>, expiration_secs: u64) -> bool {
    let elapsed: u64 = (*current_time - paste.timestamp)
        .num_seconds()
        .try_into()
        .expect("timestamp was in the future?");
    elapsed > expiration_secs
}
