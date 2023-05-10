use axum::extract::Path;
use axum::routing::{delete, get, put};
use axum::Router;

pub fn api_router() -> Router {
    Router::new()
        .route("/new", put(publish))
        .route("/:crate_name/:version/yank", delete(yank))
        .route("/:crate_name/:version/unyank", put(unyank))
        .route("/:crate_name/owners", get(list_owners))
        .route("/:crate_name/owners", delete(remove_owner))
        .route("/:crate_name/owners", put(add_owner))
        .route("/", get(search))
}

async fn publish() {
    todo!()
}

async fn yank(Path((_crate_name, _crate_version)): Path<(String, String)>) {
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
