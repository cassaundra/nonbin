use std::collections::HashMap;
use std::net::SocketAddr;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{body, Json, Router};
use serde_json::json;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::TraceLayer;

use urlencoding::encode;

use crate::controllers::paste;

use crate::error::AppError;

use crate::{controllers, App};

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
    mut app: State<App>,
    Path(key): Path<String>,
) -> crate::AppResult<Redirect> {
    if paste::is_expired(&mut app, &key).await? {
        return Err(AppError::NotFound);
    }

    let file_name = app.database.get_paste(&key).await?.file_name;

    Ok(Redirect::permanent(&format!("/{key}/{file_name}")))
}

async fn get_paste(
    mut app: State<App>,
    Path((key, _file_name)): Path<(String, String)>,
) -> crate::AppResult<Response<body::Full<Bytes>>> {
    if paste::is_expired(&mut app, &key).await? {
        return Err(AppError::NotFound);
    }

    let data = paste::fetch_data(&mut app, &key).await?;
    let response = Response::builder().body(body::Full::new(data))?;

    Ok(response)
}

async fn upload_paste(
    mut app: State<App>,
    mut multipart: Multipart,
) -> crate::AppResult<impl IntoResponse> {
    // just take the first multipart field
    if let Some(field) = multipart.next_field().await? {
        let file_name = field
            .file_name()
            .ok_or_else(|| AppError::MissingFileName)?
            .to_owned();
        let data = field.bytes().await?;

        let paste = controllers::paste::create(&mut app, &file_name, data).await?;

        let encoded_file_name = encode(&file_name);
        let path = format!("/{key}/{encoded_file_name}", key = paste.key);
        let url = format!("{base_url}{path}", base_url = app.config.base_url);

        Ok((
            StatusCode::CREATED,
            [(header::LOCATION, path)],
            Json(json!({
                "id": paste.key,
                "delete_key": paste.delete_key,
                "url": url,
            }
            )),
        ))
    } else {
        Err(AppError::MissingFile)
    }
}

async fn delete_paste(
    mut app: State<App>,
    Query(params): Query<HashMap<String, String>>,
    Path(key): Path<String>,
) -> crate::AppResult<impl IntoResponse> {
    let delete_key = params
        .get("delete_key")
        .ok_or_else(|| AppError::MissingDeleteKey)?;

    // compare against the actual delete key if it exists
    if let Some(real_delete_key) = app.database.get_paste(&key).await?.delete_key {
        if delete_key == &real_delete_key {
            paste::delete(&mut app, &key).await?;
            return Ok(());
        }
    }

    Err(AppError::WrongDeleteKey)
}
