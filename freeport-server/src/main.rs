use axum::Router;

mod routes;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let router = Router::new()
        .nest("/index", routes::index::index_router())
        .nest("/api/v1/crates/", routes::api::api_router());

    axum::Server::bind(&"0.0.0.0:1999".parse().unwrap())
        .serve(router.into_make_service())
        .await
        .unwrap()
}
