use anyhow::anyhow;
use chrono::Utc;
use tracing::{info, warn};

use crate::storage::Storage;
use crate::App;

pub async fn run(mut app: App) -> anyhow::Result<()> {
    let Some(expiration_time) = app.config.limits.expiration_time else {
        warn!("no expiration time configured, doing nothing");
        return Ok(());
    };

    let pastes = app.database.get_all_pastes().await?;

    let now = Utc::now();

    for paste in pastes {
        let elapsed: u64 = (now - paste.timestamp)
            .num_seconds()
            .try_into()
            .map_err(|_| anyhow!("time went backwards?!"))?;
        if elapsed > expiration_time {
            info!("deleting expired paste: {}", paste.key);
            app.database.delete_paste(&paste.key).await?;
            app.storage.delete_object(&paste.key).await?;
        }
    }
    Ok(())
}
