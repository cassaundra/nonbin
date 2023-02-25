use std::io::BufRead;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::{fs, io};

use anyhow::Context;
use aws_config::retry::RetryConfig;
use aws_sdk_s3 as s3;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, FromRef, Multipart, Path, State};
use axum::response::{Redirect, Response};
use axum::routing::get;
use axum::{body, Router};
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::Deserialize;
use tower_http::limit::RequestBodyLimitLayer;
use tracing::info;
use urlencoding::encode;

mod error;
pub(crate) use error::{ApiError, ApiResult};

#[derive(Clone, FromRef)]
struct AppState {
    config: Config,
    words: Words,
    s3_client: s3::Client,
}

#[derive(Debug, Clone, Deserialize)]
struct Config {
    port: u16,
    max_upload_size: usize,
    adjectives_file: PathBuf,
    nouns_file: PathBuf,
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
        .route("/:id/:file_name", get(get_paste))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(config.max_upload_size))
        .with_state(AppState {
            config,
            words,
            s3_client,
        });

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn index() -> &'static str {
    "TODO documentation"
}

async fn get_paste(
    State(config): State<Config>,
    State(s3_client): State<s3::Client>,
    Path((id, file_name)): Path<(String, String)>,
) -> crate::ApiResult<Response<body::Full<Bytes>>> {
    let sanitized_file_name = sanitize_key(&file_name);
    let object = s3_client
        .get_object()
        .bucket(&config.s3_bucket)
        .key(&format!("{id}/{sanitized_file_name}"))
        .send()
        .await?;

    let body = object.body.collect().await.unwrap().into_bytes();
    let response = Response::builder().body(body::Full::new(body))?;

    Ok(response)
}

async fn upload_paste(
    State(config): State<Config>,
    State(words): State<Words>,
    State(s3_client): State<s3::Client>,
    mut multipart: Multipart,
) -> crate::ApiResult<Redirect> {
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

        let id = generate_id(&words);
        let sanitized_file_name = sanitize_key(&file_name);

        info!(
            "uploading: id='{id}', file='{sanitized_file_name}', content_type='{content_type}', \
             size={size}",
            size = data.len()
        );

        s3_client
            .put_object()
            .bucket(&config.s3_bucket)
            .key(&format!("{id}/{sanitized_file_name}"))
            .metadata("user-file-name", &file_name)
            .metadata("user-content-type", &content_type)
            .body(data.into())
            .send()
            .await?;

        let encoded_sanitized_file_name = encode(&sanitized_file_name);
        Ok(Redirect::to(&format!(
            "/{id}/{encoded_sanitized_file_name}"
        )))
    } else {
        Err(ApiError::MissingFile)
    }
}

/// Sanitize a file name to thwart weird directory hacks.
///
/// Read more about special character handling in S3 keys:
/// https://docs.aws.amazon.com/AmazonS3/latest/userguide/object-keys.html
fn sanitize_key(key: &str) -> String {
    // As far as I can tell, this is the only special character that we need to
    // worry about, since the rest is handled by various middleware.
    key.replace('/', "_")
}

fn generate_id(words: &Words) -> String {
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
