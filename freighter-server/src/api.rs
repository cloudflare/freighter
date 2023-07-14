use crate::ServiceState;
use anyhow::Context;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Html;
use axum::routing::{delete, get, post, put};
use axum::{Form, Json, Router};
use freighter_auth::AuthProvider;
use freighter_index::{
    AuthForm, CompletedPublication, IndexProvider, ListQuery, Publish, SearchQuery, SearchResults,
    SearchResultsEntry,
};
use freighter_storage::StorageProvider;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[non_exhaustive]
#[derive(Deserialize)]
pub struct OwnerListChange {
    pub users: Vec<String>,
}

pub fn api_router<I, S, A>() -> Router<Arc<ServiceState<I, S, A>>>
where
    I: IndexProvider + Send + Sync + 'static,
    S: StorageProvider + Send + Sync + Clone + 'static,
    A: AuthProvider + Send + Sync + 'static,
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
        .route("/all", get(list))
        .fallback(handle_api_fallback)
}

async fn publish<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    mut body: Bytes,
) -> axum::response::Result<Json<CompletedPublication>>
where
    I: IndexProvider + Send + Sync,
    S: StorageProvider + Send + Sync + Clone + 'static,
    A: AuthProvider,
{
    if body.len() <= 4 {
        return Err(StatusCode::BAD_REQUEST.into());
    }

    let json_len_bytes = body.split_to(4);
    let json_len = u32::from_le_bytes(json_len_bytes.as_ref().try_into().unwrap()) as usize;

    if body.len() < json_len {
        return Err(StatusCode::BAD_REQUEST.into());
    }

    let json_bytes = body.split_to(json_len);

    if body.len() <= 4 {
        return Err(StatusCode::BAD_REQUEST.into());
    }

    let crate_len_bytes = body.split_to(4);
    let crate_len = u32::from_le_bytes(crate_len_bytes.as_ref().try_into().unwrap()) as usize;

    if body.len() < crate_len {
        return Err(StatusCode::BAD_REQUEST.into());
    }

    let crate_bytes = body.split_to(crate_len);

    let json: Publish = serde_json::from_slice(&json_bytes).map_err(|_| StatusCode::BAD_REQUEST)?;

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
        .map(Json)?;

    Ok(resp)
}

async fn yank<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path((name, version)): Path<(String, Version)>,
) -> axum::response::Result<()>
where
    I: IndexProvider,
    A: AuthProvider,
{
    let auth = headers
        .get(AUTHORIZATION)
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .or(Err(StatusCode::BAD_REQUEST))?;

    state.auth.auth_yank(auth, &name).await?;

    state.index.yank_crate(&name, &version).await?;

    Ok(())
}

async fn unyank<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path((name, version)): Path<(String, Version)>,
) -> axum::response::Result<()>
where
    I: IndexProvider,
    A: AuthProvider,
{
    let auth = headers
        .get(AUTHORIZATION)
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .or(Err(StatusCode::BAD_REQUEST))?;

    state.auth.auth_unyank(auth, &name).await?;

    state.index.unyank_crate(&name, &version).await?;

    Ok(())
}

async fn list_owners<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path(name): Path<String>,
) -> axum::response::Result<()>
where
    A: AuthProvider,
{
    let auth = headers
        .get(AUTHORIZATION)
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_str()
        .or(Err(StatusCode::BAD_REQUEST))?;

    state.auth.list_owners(auth, &name).await?;

    Ok(())
}

async fn add_owners<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path(name): Path<String>,
    Json(owners): Json<OwnerListChange>,
) -> axum::response::Result<()>
where
    A: AuthProvider,
{
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

    Ok(())
}

async fn remove_owners<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Path(name): Path<String>,
    Json(owners): Json<OwnerListChange>,
) -> axum::response::Result<()>
where
    A: AuthProvider,
{
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

    Ok(())
}

async fn register<I, S, A>(
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Form(auth): Form<AuthForm>,
) -> axum::response::Result<Html<String>>
where
    A: AuthProvider,
{
    let token = state.auth.register(&auth.username, &auth.password).await?;

    Ok(Html(token))
}

async fn login<I, S, A>(
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Form(auth): Form<AuthForm>,
) -> axum::response::Result<Html<String>>
where
    A: AuthProvider,
{
    let token = state.auth.login(&auth.username, &auth.password).await?;

    Ok(Html(token))
}

async fn search<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Query(query): Query<SearchQuery>,
) -> axum::response::Result<Json<SearchResults>>
where
    I: IndexProvider,
    A: AuthProvider + Sync,
{
    let token = headers
        .get(AUTHORIZATION)
        .map(|x| x.to_str().or(Err(StatusCode::BAD_REQUEST)))
        .transpose()?;

    state.auth.auth_view_full_index(token).await?;

    let search_results = state
        .index
        .search(&query.q, query.per_page.map(|x| x.max(100)).unwrap_or(10))
        .await?;

    Ok(Json(search_results))
}

async fn list<I, S, A>(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState<I, S, A>>>,
    Query(query): Query<ListQuery>,
) -> axum::response::Result<Json<Vec<SearchResultsEntry>>>
where
    I: IndexProvider,
    A: AuthProvider + Sync,
{
    let token = headers
        .get(AUTHORIZATION)
        .map(|x| x.to_str().or(Err(StatusCode::BAD_REQUEST)))
        .transpose()?;

    state.auth.auth_view_full_index(token).await?;

    let search_results = state.index.list(&query).await?;

    Ok(Json(search_results))
}

async fn handle_api_fallback() -> StatusCode {
    StatusCode::NOT_FOUND
}
