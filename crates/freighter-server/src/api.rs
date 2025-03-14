use crate::ServiceState;
use anyhow::Context;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post, put};
use axum::{Form, Json, Router};
use freighter_api_types::auth::request::AuthForm;
use freighter_api_types::index::request::{Publish, SearchQuery};
use freighter_api_types::index::response::{CompletedPublication, SearchResults, YankResult};
use freighter_api_types::index::IndexError;
use freighter_api_types::ownership::response::{ChangedOwnership, OwnerList};
use freighter_api_types::storage::StorageError;
use freighter_auth::AuthError;
use metrics::counter;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[non_exhaustive]
#[derive(Deserialize)]
pub struct OwnerListChange {
    pub users: Vec<String>,
}

pub fn api_router() -> Router<Arc<ServiceState>> {
    Router::new()
        .route("/new", put(publish))
        .route("/:crate_name/:version/yank", delete(yank))
        .route("/:crate_name/:version/unyank", put(unyank))
        .route("/:crate_name/owners", get(list_owners))
        .route("/:crate_name/owners", delete(remove_owners))
        .route("/:crate_name/owners", put(add_owners))
        .route("/account", post(register))
        .route("/", get(search))
        .fallback(handle_api_fallback)
}

async fn publish(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState>>,
    mut body: Bytes,
) -> axum::response::Result<Json<CompletedPublication>> {
    let auth = state
        .auth
        .token_from_headers(&headers)?
        .ok_or((StatusCode::UNAUTHORIZED, "Auth token missing"))?;

    if body.len() <= 4 {
        return Err((StatusCode::BAD_REQUEST, "Missing body").into());
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
        return Err((StatusCode::BAD_REQUEST, "Crate data truncated").into());
    }

    let crate_bytes = body.split_to(crate_len);

    let json: Publish = serde_json::from_slice(&json_bytes)
        .map_err(|_| (StatusCode::BAD_REQUEST, "JSON parsing error"))?;

    let auth_result = state.auth.publish(auth, &json.name).await;

    if let Err(e) = &auth_result {
        let error_label = match e {
            AuthError::Unauthorized => "unauthorized",
            AuthError::Forbidden => "forbidden",
            AuthError::InvalidCredentials => "invalid_credentials",
            AuthError::Unimplemented => "unimplemented",
            AuthError::CrateNotFound => "crate_not_found",
            AuthError::ServiceError(_) => "service_error",
        };

        counter!("freighter_publish_auth_errors_total", "error" => error_label).increment(1);
    }

    auth_result?;

    let version = json.vers.to_string();
    let mut stored_crate = false;
    let tarball_checksum: [u8; 32] = Sha256::digest(&crate_bytes).into();

    let res = {
        let end_step = std::pin::pin!(async {
            let res = state
                .storage
                .put_crate(&json.name, &version, crate_bytes, tarball_checksum)
                .await;

            if let Err(e) = &res {
                let error_label = match e {
                    StorageError::NotFound => "not_found",
                    StorageError::ServiceError(_) => "service_error",
                };

                counter!("freighter_publish_tarballs_errors_total", "error" => error_label)
                    .increment(1);
            }

            res.context("Failed to store the crate in a storage medium")?;

            stored_crate = true;
            Ok(())
        });
        state.index.publish(&json, tarball_checksum, end_step).await
    };

    match res {
        Ok(res) => {
            // publish() is never allowed to proceed without the end_step succeeding.
            assert!(stored_crate);
            Ok(Json(res))
        }
        Err(e) => {
            let error_label = match &e {
                IndexError::Conflict(_) => "conflict",
                IndexError::CrateNameNotAllowed => "crate_name_not_allowed",
                IndexError::NotFound => "crate_not_found",
                IndexError::ServiceError(_) => "service_error",
            };

            counter!("freighter_publish_index_errors_total", "error" => error_label).increment(1);

            if stored_crate {
                let _ = state
                    .storage
                    .delete_crate(&json.name, &version, tarball_checksum)
                    .await;
            }
            Err(e.into())
        }
    }
}

async fn yank(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState>>,
    Path((name, version)): Path<(String, Version)>,
) -> axum::response::Result<Json<YankResult>> {
    let auth = state
        .auth
        .token_from_headers(&headers)?
        .ok_or((StatusCode::UNAUTHORIZED, "Auth token missing"))?;

    state.auth.auth_yank(auth, &name).await?;

    state.index.yank_crate(&name, &version).await?;

    Ok(Json::default())
}

async fn unyank(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState>>,
    Path((name, version)): Path<(String, Version)>,
) -> axum::response::Result<Json<YankResult>> {
    let auth = state
        .auth
        .token_from_headers(&headers)?
        .ok_or((StatusCode::UNAUTHORIZED, "Auth token missing"))?;

    state.auth.auth_yank(auth, &name).await?;

    state.index.unyank_crate(&name, &version).await?;

    Ok(Json::default())
}

async fn list_owners(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState>>,
    Path(name): Path<String>,
) -> axum::response::Result<Json<OwnerList>> {
    let auth = state
        .auth
        .token_from_headers(&headers)?
        .ok_or((StatusCode::UNAUTHORIZED, "Auth token missing"))?;

    let users = state.auth.list_owners(auth, &name).await?;

    Ok(Json(OwnerList { users }))
}

async fn add_owners(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState>>,
    Path(name): Path<String>,
    Json(owners): Json<OwnerListChange>,
) -> axum::response::Result<Json<ChangedOwnership>> {
    let auth = state
        .auth
        .token_from_headers(&headers)?
        .ok_or((StatusCode::UNAUTHORIZED, "Auth token missing"))?;

    state
        .auth
        .add_owners(
            auth,
            &owners.users.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
            &name,
        )
        .await?;

    Ok(Json(ChangedOwnership::with_msg("owners successfully added".into())))
}

async fn remove_owners(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState>>,
    Path(name): Path<String>,
    Json(owners): Json<OwnerListChange>,
) -> axum::response::Result<Json<ChangedOwnership>> {
    let auth = state
        .auth
        .token_from_headers(&headers)?
        .ok_or((StatusCode::UNAUTHORIZED, "Auth token missing"))?;

    state
        .auth
        .remove_owners(
            auth,
            &owners.users.iter().map(|x| x.as_str()).collect::<Vec<_>>(),
            &name,
        )
        .await?;

    Ok(Json(ChangedOwnership::with_msg(
        "owners successfully removed".into(),
    )))
}

async fn register(
    State(state): State<Arc<ServiceState>>,
    Form(auth): Form<AuthForm>,
) -> axum::response::Result<String> {
    if !state.config.allow_registration {
        return Err((StatusCode::UNAUTHORIZED, "Registration disabled").into());
    }

    let token = state.auth.register(&auth.username).await?;
    Ok(token)
}

async fn search(
    headers: HeaderMap,
    State(state): State<Arc<ServiceState>>,
    Query(query): Query<SearchQuery>,
) -> axum::response::Result<Json<SearchResults>> {
    if state.config.auth_required {
        let token = state
            .auth
            .token_from_headers(&headers)?
            .ok_or((StatusCode::UNAUTHORIZED, "Auth token missing"))?;

        state.auth.auth_view_full_index(token).await?;
    }

    let search_results = state
        .index
        .search(&query.q, query.per_page.map_or(10, |x| x.max(100)))
        .await?;

    Ok(Json(search_results))
}

async fn handle_api_fallback() -> (StatusCode, &'static str) {
    (
        StatusCode::NOT_FOUND,
        "Freighter: Invalid URL for the crates.io API endpoint",
    )
}
