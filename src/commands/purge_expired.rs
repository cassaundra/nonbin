use crate::controllers::paste::purge_expired;
use crate::App;

pub async fn run(mut app: App) -> anyhow::Result<()> {
    purge_expired(&mut app).await?;
    Ok(())
}
