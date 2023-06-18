use crate::model::ServiceState;
use anyhow::Context;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Html;
use axum::routing::{delete, get, post, put};
use axum::{Form, Json, Router};
use freighter_auth::AuthClient;
use freighter_index::{
    AuthForm, CompletedPublication, IndexClient, Publish, SearchQuery, SearchResults,
};
use freighter_storage::StorageClient;
use metrics::histogram;
use semver::Version;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Instant;

pub fn api_router<I, S, A>() -> Router<Arc<ServiceState<I, S, A>>>
where
    I: IndexClient + Send + Sync + 'static,
    S: StorageClient + Send + Sync + Clone + 'static,
    A: AuthClient + Send + Sync + 'static,
{
    Router::new()
        .route("/new", put(publish))
        .route("/:crate_name/:version/yank", delete(yank))
        .route("/:crate_name/:version/unyank", put(unyank))
        .route("/:crate_name/owners", get(list_owners))
        .route("/:crate_name/owners", delete(remove_owner))
        .route("/:crate_name/owners", put(add_owner))
        .route("/account", post(register))
        .route("/account/token", post(login))
        .route("/", get(search))
        .fallback(handle_api_fallback)
}

// todo don't panic on bad input
async fn publish<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    mut body: Bytes,
) -> Result<Json<CompletedPublication>, StatusCode>
where
    I: IndexClient + Send + Sync,
    S: StorageClient + Send + Sync + Clone + 'static,
    A: AuthClient,
{
    let timer = Instant::now();

    let json_len_bytes = body.split_to(4);
    let json_len = u32::from_le_bytes(json_len_bytes.as_ref().try_into().unwrap());

    let json_bytes = body.split_to(json_len as usize);

    let crate_len_bytes = body.split_to(4);
    let crate_len = u32::from_le_bytes(crate_len_bytes.as_ref().try_into().unwrap());

    let crate_bytes = body.split_to(crate_len as usize);

    let json: Publish = serde_json::from_slice(&json_bytes).unwrap();

    let resp = if {
        if let Some(auth) = headers
            .get(AUTHORIZATION)
            .map(|header| header.to_str().ok())
            .flatten()
        {
            state.auth.publish(auth, &json.name).await.is_err()
        } else {
            false
        }
    } {
        let hash = format!("{:x}", Sha256::digest(&crate_bytes));

        let name = json.name.clone();
        let version = json.vers.to_string();
        let storage = state.storage.clone();

        let resp = state
            .index
            .publish(
                &json,
                &hash,
                Box::pin(async move {
                    storage
                        .put_crate(&name, &version, &crate_bytes)
                        .await
                        .context("Failed to store crate in storage medium")
                }),
            )
            .await
            .map(|x| Json(x));

        resp.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    };

    let elapsed = timer.elapsed();

    let code = if let Err(e) = &resp {
        e.as_u16().to_string()
    } else {
        "200".to_string()
    };

    histogram!("request_duration_seconds", elapsed, "code" => code, "endpoint" => "publish");

    resp
}

async fn yank<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path((name, version)): Path<(String, Version)>,
) -> StatusCode
where
    I: IndexClient,
    A: AuthClient,
{
    let timer = Instant::now();

    let code = if let Some(auth) = headers
        .get(AUTHORIZATION)
        .map(|header| header.to_str().ok())
        .flatten()
    {
        if state.auth.auth_yank(auth, &name).await.is_ok() {
            if state.index.yank_crate(&name, &version).await.is_ok() {
                StatusCode::OK
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        }
    } else {
        StatusCode::BAD_REQUEST
    };

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "code" => code.as_u16().to_string(), "endpoint" => "yank");

    code
}

async fn unyank<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path((name, version)): Path<(String, Version)>,
) -> StatusCode
where
    I: IndexClient,
    A: AuthClient,
{
    let timer = Instant::now();

    let code = if let Some(_auth) = headers
        .get(AUTHORIZATION)
        .map(|header| header.to_str().ok())
        .flatten()
    {
        if state.index.unyank_crate(&name, &version).await.is_ok() {
            StatusCode::OK
        } else {
            StatusCode::UNAUTHORIZED
        }
    } else {
        StatusCode::BAD_REQUEST
    };

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "code" => code.as_u16().to_string(), "endpoint" => "unyank");

    code
}

async fn list_owners() {
    todo!()
}

async fn add_owner() {
    todo!()
}

async fn remove_owner() {
    todo!()
}

async fn register<I, S, A>(
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Form(auth): Form<AuthForm>,
) -> Result<Html<String>, StatusCode>
where
    A: AuthClient,
{
    if let Ok(token) = state.auth.register(&auth.username, &auth.password).await {
        Ok(Html(token))
    } else {
        Err(StatusCode::CONFLICT)
    }
}

async fn login<I, S, A>(
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Form(auth): Form<AuthForm>,
) -> Result<Html<String>, StatusCode>
where
    A: AuthClient,
{
    if let Ok(token) = state.auth.login(&auth.username, &auth.password).await {
        Ok(Html(token))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

async fn search<I, S, A>(
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Query(query): Query<SearchQuery>,
) -> Json<SearchResults>
where
    I: IndexClient,
{
    let timer = Instant::now();

    let resp = Json(
        state
            .index
            .search(&query.q, query.per_page.map(|x| x.max(100)).unwrap_or(10))
            .await
            .unwrap(),
    );

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "code" => "200", "endpoint" => "search");

    resp
}

async fn handle_api_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}
