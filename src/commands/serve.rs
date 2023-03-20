use std::collections::HashMap;
use std::net::SocketAddr;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{body, Json, Router};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use urlencoding::encode;
use uuid::Uuid;

use crate::config::Config;
use crate::db::Database;
use crate::error::ApiError;
use crate::storage::{AnyStorage, Storage};
use crate::types::api::UploadPaste;
use crate::words::{generate_key, WordLists};
use crate::App;

/// The manual for the program in man page form.
const MAN_PAGE: &str = include_str!("../../assets/man.txt");

pub async fn run(app: App) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([127, 0, 0, 1], app.config.port));

    let app = Router::new()
        .route("/", get(index).post(upload_paste))
        .route("/:id", get(get_paste_bare).delete(delete_paste))
        .route("/:id/", get(get_paste_bare).delete(delete_paste)) // hack
        .route("/:id/:file_name", get(get_paste))
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(
            app.config.limits.max_upload_size,
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(app);

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
