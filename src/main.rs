#![feature(async_fn_in_trait)]
#![feature(io_error_more)]
#![allow(incomplete_features)]

use anyhow::Context;
use axum::extract::FromRef;
use clap::{Parser, Subcommand};
use tokio::fs;

use crate::config::Config;
use crate::db::Database;
use crate::storage::AnyStorage;
use crate::words::WordLists;

mod config;
mod db;
mod error;
mod server;
mod storage;
mod types;
mod words;

pub use crate::error::ApiResult;

#[derive(Debug, Parser)]
#[command(color = clap::ColorChoice::Never)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Serve,
}

#[derive(Clone, FromRef)]
pub struct App {
    config: Config,
    database: Database,
    storage: AnyStorage,
    word_lists: WordLists,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let config: Config = {
        let contents = fs::read_to_string("config.toml")
            .await
            .context("failed to read config file")?;
        toml::from_str(&contents).context("failed to parse config file")?
    };

    let database = Database::connect(&config.database.url).await?;

    let storage = match &config.storage.kind {
        config::StorageKind::File => storage::file::FileStorage::new(&config.storage.file.dir)
            .await?
            .into(),
        #[cfg(feature = "s3")]
        config::StorageKind::S3 => {
            let s3_config = &config.storage.s3;
            storage::s3::S3Storage::new(
                &s3_config.bucket,
                s3_config.region.as_deref(),
                s3_config.endpoint.as_deref(),
            )
            .await
            .into()
        }
    };

    let word_lists = WordLists::load(
        &config.word_lists.adjectives_file,
        &config.word_lists.nouns_file,
    )
    .await?;

    let app = App {
        config,
        database,
        storage,
        word_lists,
    };

    match &args.command {
        Command::Serve => server::serve(app).await?,
    }

    Ok(())
}
