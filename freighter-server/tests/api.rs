pub mod common;

use crate::common::{
    utils::{crate_version, generate_crate_payload},
    MockIndexProvider, ServiceStateBuilder,
};

use axum::http::{Request, StatusCode};
use freighter_server::api;
use hyper::{header::AUTHORIZATION, Body};
use serde_json::Value;
use std::collections::BTreeMap;
use tower::ServiceExt;

#[tokio::test]
async fn publish_crate() {
    let router = api::api_router();

    const TOKEN: &str = "12345";

    let state = ServiceStateBuilder::default()
        .auth_provider(common::MockAuthProvider {
            valid_tokens: [TOKEN.to_owned()].into(),
        })
        .build();

    let payload = generate_crate_payload("example-lib", "1.0.1", &[1u8; 100]);

    let response = router
        .with_state(state)
        .oneshot(
            Request::builder()
                .uri("/new")
                .method("PUT")
                .header(AUTHORIZATION, TOKEN)
                .body(payload.into())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn publish_crate_auth_denied() {
    let router = api::api_router();

    let state = ServiceStateBuilder::default()
        .auth_provider(common::MockAuthProvider {
            valid_tokens: [].into(),
        })
        .build();

    let payload = generate_crate_payload("example-lib", "1.0.1", &[1u8; 100]);

    let response = router
        .with_state(state)
        .oneshot(
            Request::builder()
                .uri("/new")
                .method("PUT")
                .header(AUTHORIZATION, "1234")
                .body(payload.into())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_all_crates() {
    let router = api::api_router();

    let crates = BTreeMap::from([
        (
            "example-lib".to_owned(),
            ["1.3.0", "1.3.1"]
                .iter()
                .map(|version| crate_version("example-lib", version))
                .collect::<Vec<_>>(),
        ),
        (
            "freighter".to_owned(),
            ["0.1.0", "0.1.1"]
                .iter()
                .map(|version| crate_version("freighter", version))
                .collect::<Vec<_>>(),
        ),
    ]);

    let state = ServiceStateBuilder::default()
        .index_provider(MockIndexProvider { crates })
        .build();

    let response = router
        .with_state(state)
        .oneshot(Request::builder().uri("/all").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        value,
        serde_json::json!([
          {
            "name": "example-lib",
            "max_version": "1.3.1",
            "description": "Description example-lib 1.3.1",
            "homepage": "e.com",
            "repository": "ssh://git@b.com/a/f.git",
            "documentation": null,
            "keywords": [ "example" ],
            "categories": [ "a", "x" ]
          },
          {
            "name": "freighter",
            "max_version": "0.1.1",
            "description": "Description freighter 0.1.1",
            "homepage": "e.com",
            "repository": "ssh://git@b.com/a/f.git",
            "documentation": null,
            "keywords": [ "example" ],
            "categories": [ "a", "x" ]
          }
        ])
    );
}
