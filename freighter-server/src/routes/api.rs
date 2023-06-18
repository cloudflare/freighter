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
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Instant;

#[non_exhaustive]
#[derive(Deserialize)]
pub struct OwnerListChange {
    pub users: Vec<String>,
}

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
        .route("/:crate_name/owners", delete(remove_owners))
        .route("/:crate_name/owners", put(add_owners))
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
) -> axum::response::Result<Json<CompletedPublication>>
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

    let auth = headers
        .get(AUTHORIZATION)
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .or(Err(StatusCode::BAD_REQUEST))?;

    state.auth.publish(auth, &json.name).await?;

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
        .map(|x| Json(x))?;

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "endpoint" => "publish");

    Ok(resp)
}

async fn yank<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path((name, version)): Path<(String, Version)>,
) -> axum::response::Result<()>
where
    I: IndexClient,
    A: AuthClient,
{
    let timer = Instant::now();

    let auth = headers
        .get(AUTHORIZATION)
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .or(Err(StatusCode::BAD_REQUEST))?;

    state.auth.auth_yank(auth, &name).await?;

    state.index.yank_crate(&name, &version).await?;

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "endpoint" => "yank");

    Ok(())
}

async fn unyank<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path((name, version)): Path<(String, Version)>,
) -> axum::response::Result<()>
where
    I: IndexClient,
    A: AuthClient,
{
    let timer = Instant::now();

    let auth = headers
        .get(AUTHORIZATION)
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .or(Err(StatusCode::BAD_REQUEST))?;

    state.auth.auth_unyank(auth, &name).await?;

    state.index.unyank_crate(&name, &version).await?;

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "endpoint" => "unyank");

    Ok(())
}

async fn list_owners<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path(name): Path<String>,
) -> axum::response::Result<()>
where
    A: AuthClient,
{
    let timer = Instant::now();

    let auth = headers
        .get(AUTHORIZATION)
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .or(Err(StatusCode::BAD_REQUEST))?;

    state.auth.list_owners(auth, &name).await?;

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "endpoint" => "list_owners");

    Ok(())
}

async fn add_owners<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path(name): Path<String>,
    Json(owners): Json<OwnerListChange>,
) -> axum::response::Result<()>
where
    A: AuthClient,
{
    let timer = Instant::now();

    let auth = headers
        .get(AUTHORIZATION)
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .or(Err(StatusCode::BAD_REQUEST))?;

    state
        .auth
        .add_owners(
            auth,
            &owners.users.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
            &name,
        )
        .await?;

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "endpoint" => "add_owners");

    Ok(())
}

async fn remove_owners<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path(name): Path<String>,
    Json(owners): Json<OwnerListChange>,
) -> axum::response::Result<()>
where
    A: AuthClient,
{
    let timer = Instant::now();

    let auth = headers
        .get(AUTHORIZATION)
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .or(Err(StatusCode::BAD_REQUEST))?;

    state
        .auth
        .remove_owners(
            auth,
            &owners.users.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
            &name,
        )
        .await?;

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "endpoint" => "remove_owners");

    Ok(())
}

async fn register<I, S, A>(
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Form(auth): Form<AuthForm>,
) -> axum::response::Result<Html<String>>
where
    A: AuthClient,
{
    let timer = Instant::now();

    let token = state.auth.register(&auth.username, &auth.password).await?;

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "endpoint" => "register");

    Ok(Html(token))
}

async fn login<I, S, A>(
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Form(auth): Form<AuthForm>,
) -> axum::response::Result<Html<String>>
where
    A: AuthClient,
{
    let timer = Instant::now();

    let token = state.auth.login(&auth.username, &auth.password).await?;

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "endpoint" => "login");

    Ok(Html(token))
}
async fn search<I, S, A>(
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Query(query): Query<SearchQuery>,
) -> axum::response::Result<Json<SearchResults>>
where
    I: IndexClient,
{
    let timer = Instant::now();

    let search_results = state
        .index
        .search(&query.q, query.per_page.map(|x| x.max(100)).unwrap_or(10))
        .await?;

    let elapsed = timer.elapsed();

    histogram!("request_duration_seconds", elapsed, "endpoint" => "search");

    Ok(Json(search_results))
}

async fn handle_api_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}
