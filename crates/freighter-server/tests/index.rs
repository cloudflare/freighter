pub mod common;

use crate::common::utils::crate_version;
use crate::common::{MockIndexProvider, ServiceStateBuilder};
use axum::body::to_bytes;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use freighter_server::index;
use std::collections::BTreeMap;
use tower::ServiceExt;

#[tokio::test]
async fn index_config_endpoint() {
    let _ = tracing_subscriber::fmt::fmt().with_test_writer().try_init();

    let router = index::index_router();

    let state = ServiceStateBuilder::default().build();

    let response = router
        .with_state(state.clone())
        .oneshot(
            Request::builder()
                .uri("/config.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 100_000).await.unwrap();
    let body = std::str::from_utf8(&body).unwrap();
    assert_eq!(
        body,
        &format!(
            r#"{{"dl":"{}","api":"{}","auth-required":false}}"#,
            state.config.download_endpoint, state.config.api_endpoint
        )
    );
}

#[tokio::test]
async fn missing_crate() {
    let _ = tracing_subscriber::fmt::fmt().with_test_writer().try_init();

    let router = index::index_router();

    let state = ServiceStateBuilder::default().build();

    let response = router
        .with_state(state)
        .oneshot(
            Request::builder()
                .uri("/ex/am/example-lib")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn valid_crate() {
    let _ = tracing_subscriber::fmt::fmt().with_test_writer().try_init();

    let router = index::index_router();

    let crates = BTreeMap::from([(
        "example-lib".to_owned(),
        ["1.3.0", "1.3.1"]
            .iter()
            .map(|version| crate_version("example-lib", version))
            .collect::<Vec<_>>(),
    )]);

    let state = ServiceStateBuilder::default()
        .index_provider(MockIndexProvider { crates })
        .build();

    let response = router
        .with_state(state)
        .oneshot(
            Request::builder()
                .uri("/ex/am/example-lib")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 100_000).await.unwrap();
    let body = std::str::from_utf8(&body).unwrap();
    assert_eq!(
        body,
        r#"{"name":"example-lib","vers":"1.3.0","deps":[],"cksum":"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff","features":{},"v":2}
{"name":"example-lib","vers":"1.3.1","deps":[],"cksum":"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff","features":{},"v":2}
"#
    );
}
