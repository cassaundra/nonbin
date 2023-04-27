use crate::controllers::paste;
use crate::App;

pub async fn run(mut app: App) -> anyhow::Result<()> {
    paste::purge_expired(&mut app).await?;
    Ok(())
}
