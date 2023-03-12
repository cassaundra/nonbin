#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

use std::collections::HashMap;
use std::net::SocketAddr;

use anyhow::Context;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, FromRef, Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{body, Json, Router};
use rand::seq::SliceRandom;
use rand::thread_rng;
use tokio::io::AsyncBufReadExt;
use tokio::{fs, io};
use tokio_stream::wrappers::LinesStream;
use tokio_stream::StreamExt;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::normalize_path::NormalizePathLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use urlencoding::encode;
use uuid::Uuid;

mod config;
use crate::config::Config;

mod db;
use db::Database;

mod error;
pub(crate) use error::{ApiError, ApiResult};

pub(crate) mod storage;
use storage::{AnyStorage, Storage};

pub(crate) mod types;
use types::api::UploadPaste;

/// The manual for the program in man page form.
const MAN_PAGE: &str = include_str!("../assets/man.txt");

#[derive(Clone, FromRef)]
struct AppState {
    config: Config,
    database: Database,
    storage: AnyStorage,
    word_lists: WordLists,
}

#[derive(Debug, Clone)]
struct WordLists {
    adjectives: Vec<String>,
    nouns: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config: Config = {
        let contents = fs::read_to_string("config.toml")
            .await
            .context("failed to read config file")?;
        toml::from_str(&contents).context("failed to parse config file")?
    };

    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

    let database = Database::connect(&config.database.url).await?;

    let s3_storage = storage::s3::S3Storage::new(
        config.storage.s3.region.clone(),
        config.storage.s3.bucket.clone(),
        config.storage.s3.endpoint.clone(),
    )
    .await;

    let storage = AnyStorage::S3(s3_storage);

    let word_lists = WordLists {
        adjectives: read_lines(&config.word_lists.adjectives_file)
            .await
            .context("failed to read adjectives")?,
        nouns: read_lines(&config.word_lists.nouns_file)
            .await
            .context("failed to read nouns")?,
    };

    let app = Router::new()
        .route("/", get(index).post(upload_paste))
        .route("/:id", get(get_paste_bare).delete(delete_paste))
        .route("/:id/:file_name", get(get_paste))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(config.limits.max_upload_size))
        .layer(TraceLayer::new_for_http())
        .route_layer(NormalizePathLayer::trim_trailing_slash())
        .with_state(AppState {
            config,
            word_lists,
            database,
            storage,
        });

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn index() -> &'static str {
    MAN_PAGE
}

async fn get_paste_bare(
    State(mut db): State<Database>,
    Path(key): Path<String>,
) -> crate::ApiResult<Redirect> {
    let file_name = db.get_paste(&key).await?.file_name;
    Ok(Redirect::permanent(&format!("/{key}/{file_name}")))
}

async fn get_paste(
    State(mut storage): State<AnyStorage>,
    Path((key, _file_name)): Path<(String, String)>,
) -> crate::ApiResult<Response<body::Full<Bytes>>> {
    let body = storage.get_object(&key).await?;
    let response = Response::builder().body(body::Full::new(body))?;
    Ok(response)
}

async fn upload_paste(
    State(config): State<Config>,
    State(words): State<WordLists>,
    State(mut db): State<Database>,
    State(mut storage): State<AnyStorage>,
    mut multipart: Multipart,
) -> crate::ApiResult<impl IntoResponse> {
    // just take the first multipart field
    if let Some(field) = multipart.next_field().await? {
        let file_name = field
            .file_name()
            .ok_or_else(|| ApiError::MissingFileName)?
            .to_owned();
        let content_type = field
            .content_type()
            .ok_or_else(|| ApiError::MissingFileContentType)?
            .to_owned();
        let data = field.bytes().await?;

        let key = generate_key(&words);
        let delete_key = Uuid::new_v4().to_string();

        info!(
            "uploading: key='{key}', file='{file_name}', content_type='{content_type}', \
             size={size}",
            size = data.len()
        );

        storage.put_object(&key, data).await?;
        db.insert_paste(&key, &delete_key, &file_name).await?;

        let encoded_file_name = encode(&file_name);
        let path = format!("/{key}/{encoded_file_name}");
        let url = format!("{base_url}{path}", base_url = config.base_url);

        Ok((
            StatusCode::CREATED,
            [(header::LOCATION, path)],
            Json(UploadPaste {
                id: key,
                url,
                delete_key,
            }),
        ))
    } else {
        Err(ApiError::MissingFile)
    }
}

async fn delete_paste(
    State(mut db): State<Database>,
    State(mut storage): State<AnyStorage>,
    Query(params): Query<HashMap<String, String>>,
    Path(key): Path<String>,
) -> crate::ApiResult<impl IntoResponse> {
    let delete_key = params
        .get("delete_key")
        .ok_or_else(|| ApiError::MissingDeleteKey)?;

    // compare against the actual delete key if it exists
    if let Some(real_delete_key) = db.get_paste(&key).await?.delete_key {
        if delete_key == &real_delete_key {
            storage.delete_object(&key).await?;
            db.delete_paste(&key).await?;
            return Ok(());
        }
    }

    Err(ApiError::WrongDeleteKey)
}

fn generate_key(words: &WordLists) -> String {
    let mut rng = thread_rng();
    let adj_a = words.adjectives.choose(&mut rng).unwrap();
    let adj_b = words.adjectives.choose(&mut rng).unwrap();
    let noun = words.nouns.choose(&mut rng).unwrap();
    format!("{adj_a}-{adj_b}-{noun}")
}

async fn read_lines(path: impl AsRef<std::path::Path>) -> io::Result<Vec<String>> {
    let file = fs::File::open(path).await?;
    let lines = LinesStream::new(io::BufReader::new(file).lines())
        .filter_map(|s| s.ok())
        .filter(|s| !s.is_empty())
        .collect()
        .await;
    Ok(lines)
}
