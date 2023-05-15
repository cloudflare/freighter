use crate::model::ServiceState;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::routing::{delete, get, put};
use axum::Router;
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub fn api_router() -> Router<Arc<ServiceState>> {
    Router::new()
        .route("/new", put(publish))
        .route("/:crate_name/:version/yank", delete(yank))
        .route("/:crate_name/:version/unyank", put(unyank))
        .route("/:crate_name/owners", get(list_owners))
        .route("/:crate_name/owners", delete(remove_owner))
        .route("/:crate_name/owners", put(add_owner))
        .route("/", get(search))
        .fallback(handle_api_fallback)
}

// todo don't panic
async fn publish(State(state): State<Arc<ServiceState>>, mut body: Bytes) {
    let json_len_bytes = body.split_to(4);
    let json_len = u32::from_le_bytes(json_len_bytes.as_ref().try_into().unwrap());

    let json_bytes = body.split_to(json_len as usize);

    let crate_len_bytes = body.split_to(4);
    let crate_len = u32::from_le_bytes(crate_len_bytes.as_ref().try_into().unwrap());

    let crate_bytes = body.split_to(crate_len as usize);

    let json = serde_json::from_slice(&json_bytes).unwrap();

    let hash = format!("{:x}", Sha256::digest(&crate_bytes));

    state
        .publish_crate(&json, &hash, &crate_bytes)
        .await
        .unwrap();
}

async fn yank() {
    todo!()
}

async fn unyank() {
    todo!()
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

async fn search() {
    todo!()
}

async fn handle_api_fallback(method: Method, uri: Uri, headers: HeaderMap) -> StatusCode {
    tracing::error!(
        ?method,
        ?uri,
        ?headers,
        "Could not match request with any routes on api"
    );

    StatusCode::NOT_FOUND
}
