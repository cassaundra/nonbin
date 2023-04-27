#![feature(async_fn_in_trait)]
#![feature(io_error_more)]
#![feature(return_position_impl_trait_in_trait)]
#![allow(incomplete_features)]

use std::path::PathBuf;

use anyhow::{bail, Context};
use axum::extract::FromRef;
use clap::{Parser, Subcommand};
use directories_next::ProjectDirs;
use tokio::fs;
use tracing::info;

use crate::config::Config;
use crate::db::Database;
use crate::storage::AnyStorage;
use crate::words::WordLists;

mod commands;
mod config;
mod controllers;
mod db;
mod error;
mod markdown;
mod models;
mod storage;
mod words;

pub use crate::error::AppResult;

#[derive(Debug, Parser)]
#[command(color = clap::ColorChoice::Never)]
struct Args {
    /// Path the config file to use
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    PurgeExpired,
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
        let path = if let Some(path) = args.config {
            path
        } else {
            let options = vec![
                Some(PathBuf::from("config.toml")),
                ProjectDirs::from("in", "nonb", "nonbin")
                    .map(|p| p.config_dir().join("config.toml")),
            ];

            match options.into_iter().flatten().find(|p| p.exists()) {
                Some(path) => path,
                _ => bail!("could not locate config.toml"),
            }
        };
        info!("reading config from {path:?}");
        let contents = fs::read_to_string(path)
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
        Command::PurgeExpired => commands::purge_expired::run(app).await?,
        Command::Serve => commands::serve::run(app).await?,
    }

    Ok(())
}
