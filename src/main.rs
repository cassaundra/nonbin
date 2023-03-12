use std::collections::HashMap;
use std::io::BufRead;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::{fs, io};

use anyhow::Context;
use aws_config::retry::RetryConfig;
use aws_sdk_s3 as s3;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, FromRef, Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{body, Json, Router};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::Deserialize;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::normalize_path::NormalizePathLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use urlencoding::encode;
use uuid::Uuid;

mod db;
use db::Database;

mod error;
pub(crate) use error::{ApiError, ApiResult};

pub(crate) mod types;
use types::api::UploadPaste;

/// The manual for the program in man page form.
const MAN_PAGE: &str = include_str!("../assets/man.txt");

#[derive(Clone, FromRef)]
struct AppState {
    config: Config,
    words: Words,
    database: Database,
    s3_client: s3::Client,
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    base_url: String,
    port: u16,
    max_upload_size: usize,
    adjectives_file: PathBuf,
    nouns_file: PathBuf,
    database_url: String,
    s3_bucket: String,
    s3_region: String,
    s3_endpoint: Option<String>,
}

#[derive(Debug, Clone)]
struct Words {
    adjectives: Vec<String>,
    nouns: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // try to load .env, ignoring any errors
    _ = dotenvy::dotenv();

    tracing_subscriber::fmt::init();

    let config: Config = config::Config::builder()
        .add_source(config::File::with_name("config.toml").required(false))
        .add_source(config::Environment::with_prefix("NONBIN"))
        .build()
        .context("failed to read config")?
        .try_deserialize()
        .context("failed to deserialize config")?;

    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

    let words = Words {
        adjectives: read_lines(&config.adjectives_file).context("failed to read adjectives")?,
        nouns: read_lines(&config.nouns_file).context("failed to read nouns")?,
    };

    let database = Database::connect(&config.database_url).await?;

    let s3_client = {
        let sdk_config = aws_config::load_from_env().await;
        let mut config_builder = s3::config::Builder::from(&sdk_config)
            .region(s3::Region::new(config.s3_region.clone()))
            .retry_config(RetryConfig::disabled());

        if let Some(endpoint) = &config.s3_endpoint {
            config_builder = config_builder.endpoint_url(endpoint);
        }

        let s3_config = config_builder.build();

        s3::Client::from_conf(s3_config)
    };

    let app = Router::new()
        .route("/", get(index).post(upload_paste))
        .route("/:id", get(get_paste_bare).delete(delete_paste))
        .route("/:id/:file_name", get(get_paste))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(config.max_upload_size))
        .layer(TraceLayer::new_for_http())
        .route_layer(NormalizePathLayer::trim_trailing_slash())
        .with_state(AppState {
            config,
            words,
            database,
            s3_client,
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
    State(config): State<Config>,
    State(s3_client): State<s3::Client>,
    Path((key, _file_name)): Path<(String, String)>,
) -> crate::ApiResult<Response<body::Full<Bytes>>> {
    let object = s3_client
        .get_object()
        .bucket(&config.s3_bucket)
        .key(&key)
        .send()
        .await?;

    let body = object.body.collect().await.unwrap().into_bytes();
    let response = Response::builder().body(body::Full::new(body))?;

    Ok(response)
}

async fn upload_paste(
    State(config): State<Config>,
    State(words): State<Words>,
    State(mut db): State<Database>,
    State(s3_client): State<s3::Client>,
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

        s3_client
            .put_object()
            .bucket(&config.s3_bucket)
            .key(&key)
            .body(data.into())
            .send()
            .await?;

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
    State(config): State<Config>,
    State(mut db): State<Database>,
    State(s3_client): State<s3::Client>,
    Query(params): Query<HashMap<String, String>>,
    Path(key): Path<String>,
) -> crate::ApiResult<impl IntoResponse> {
    let delete_key = params
        .get("delete_key")
        .ok_or_else(|| ApiError::MissingDeleteKey)?;

    // compare against the actual delete key if it exists
    if let Some(real_delete_key) = db.get_paste(&key).await?.delete_key {
        if delete_key == &real_delete_key {
            s3_client
                .delete_object()
                .bucket(&config.s3_bucket)
                .key(&key)
                .send()
                .await?;
            db.delete_paste(&key).await?;
            return Ok(());
        }
    }

    Err(ApiError::WrongDeleteKey)
}

fn generate_key(words: &Words) -> String {
    let mut rng = thread_rng();
    let adj_a = words.adjectives.choose(&mut rng).unwrap();
    let adj_b = words.adjectives.choose(&mut rng).unwrap();
    let noun = words.nouns.choose(&mut rng).unwrap();
    format!("{adj_a}-{adj_b}-{noun}")
}

fn read_lines(path: impl AsRef<std::path::Path>) -> io::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let lines = io::BufReader::new(file)
        .lines()
        .filter_map(|s| s.ok())
        .filter(|s| !s.is_empty())
        .collect();
    Ok(lines)
}
