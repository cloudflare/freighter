pub mod common;

use crate::common::{
    utils::{crate_version, generate_crate_payload},
    MockIndexProvider, ServiceStateBuilder,
};

use axum::http::{Request, StatusCode};
use freighter_api_types::index::response::CrateVersion;
use freighter_server::{api, router};
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

    let payload = generate_crate_payload("example-lib", "1.0.1", &[1u8; 100], &[]);

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

    let payload = generate_crate_payload("example-lib", "1.0.1", &[1u8; 100], &[]);

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
        .build_no_arc();

    let router = router(state.config, state.index, state.storage, state.auth);

    let response = router
        .oneshot(Request::builder().uri("/all").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        value,
        serde_json::json!(
        {
            "results": [
                {
                    "name": "example-lib",
                    "versions": [
                        {
                            "version": "1.3.0",
                        },
                        {
                            "version": "1.3.1",
                        }
                    ],
                    "created_at": "1970-01-01T00:00:00Z",
                    "updated_at": "1970-01-01T00:00:00Z",
                    "description": "Description example-lib",
                    "homepage": "e.com",
                    "repository": "ssh://git@b.com/a/f.git",
                    "documentation": null,
                    "keywords": [ "example" ],
                    "categories": [ "a", "x" ]
                },
                {
                    "name": "freighter",
                    "versions": [
                        {
                            "version": "0.1.0",
                        },
                        {
                            "version": "0.1.1",
                        }
                    ],
                    "created_at": "1970-01-01T00:00:00Z",
                    "updated_at": "1970-01-01T00:00:00Z",
                    "description": "Description freighter",
                    "homepage": "e.com",
                    "repository": "ssh://git@b.com/a/f.git",
                    "documentation": null,
                    "keywords": [ "example" ],
                    "categories": [ "a", "x" ]
                }
            ]
        })
    );
}

#[tokio::test]
async fn one_char_crate_name() {
    const CRATE_NAMES: &[&str] = &["a", "aa", "aaa", "aaaa", "aa-aa", "aa_aa"];

    let crates = BTreeMap::from_iter(
        CRATE_NAMES
            .iter()
            .map(|x| (x.to_string(), vec![crate_version(x, "1.0.0")])),
    );

    let state = ServiceStateBuilder::default()
        .index_provider(MockIndexProvider {
            crates: crates.clone(),
        })
        .build_no_arc();

    let router = router(state.config, state.index, state.storage, state.auth);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/index/1/a")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // only one version present, this should deserialize
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let value: CrateVersion = serde_json::from_slice(&body).unwrap();

    assert_eq!("a", value.name);
}

#[tokio::test]
async fn two_char_crate_name() {
    const CRATE_NAMES: &[&str] = &["a", "aa", "aaa", "aaaa", "aa-aa", "aa_aa"];

    let crates = BTreeMap::from_iter(
        CRATE_NAMES
            .iter()
            .map(|x| (x.to_string(), vec![crate_version(x, "1.0.0")])),
    );

    let state = ServiceStateBuilder::default()
        .index_provider(MockIndexProvider {
            crates: crates.clone(),
        })
        .build_no_arc();

    let router = router(state.config, state.index, state.storage, state.auth);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/index/2/aa")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // only one version present, this should deserialize
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let value: CrateVersion = serde_json::from_slice(&body).unwrap();

    assert_eq!("aa", value.name);
}

#[tokio::test]
async fn three_char_crate_name() {
    const CRATE_NAMES: &[&str] = &["a", "aa", "aaa", "aaaa", "aa-aa", "aa_aa"];

    let crates = BTreeMap::from_iter(
        CRATE_NAMES
            .iter()
            .map(|x| (x.to_string(), vec![crate_version(x, "1.0.0")])),
    );

    let state = ServiceStateBuilder::default()
        .index_provider(MockIndexProvider {
            crates: crates.clone(),
        })
        .build_no_arc();

    let router = router(state.config, state.index, state.storage, state.auth);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/index/3/a/aaa")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // only one version present, this should deserialize
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let value: CrateVersion = serde_json::from_slice(&body).unwrap();

    assert_eq!("aaa", value.name);
}

#[tokio::test]
async fn four_char_crate_name() {
    const CRATE_NAMES: &[&str] = &["a", "aa", "aaa", "aaaa", "aa-aa", "aa_aa"];

    let crates = BTreeMap::from_iter(
        CRATE_NAMES
            .iter()
            .map(|x| (x.to_string(), vec![crate_version(x, "1.0.0")])),
    );

    let state = ServiceStateBuilder::default()
        .index_provider(MockIndexProvider {
            crates: crates.clone(),
        })
        .build_no_arc();

    let router = router(state.config, state.index, state.storage, state.auth);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/index/aa/aa/aaaa")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // only one version present, this should deserialize
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let value: CrateVersion = serde_json::from_slice(&body).unwrap();

    assert_eq!("aaaa", value.name);
}

#[tokio::test]
async fn hyphen_crate_name() {
    const CRATE_NAMES: &[&str] = &["a", "aa", "aaa", "aaaa", "aa-aa", "aa_aa"];

    let crates = BTreeMap::from_iter(
        CRATE_NAMES
            .iter()
            .map(|x| (x.to_string(), vec![crate_version(x, "1.0.0")])),
    );

    let state = ServiceStateBuilder::default()
        .index_provider(MockIndexProvider {
            crates: crates.clone(),
        })
        .build_no_arc();

    let router = router(state.config, state.index, state.storage, state.auth);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/index/aa/-a/aa-aa")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // only one version present, this should deserialize
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let value: CrateVersion = serde_json::from_slice(&body).unwrap();

    assert_eq!("aa-aa", value.name);
}

#[tokio::test]
async fn underscore_crate_name() {
    const CRATE_NAMES: &[&str] = &["a", "aa", "aaa", "aaaa", "aa-aa", "aa_aa"];

    let crates = BTreeMap::from_iter(
        CRATE_NAMES
            .iter()
            .map(|x| (x.to_string(), vec![crate_version(x, "1.0.0")])),
    );

    let state = ServiceStateBuilder::default()
        .index_provider(MockIndexProvider {
            crates: crates.clone(),
        })
        .build_no_arc();

    let router = router(state.config, state.index, state.storage, state.auth);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/index/aa/_a/aa_aa")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // only one version present, this should deserialize
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let value: CrateVersion = serde_json::from_slice(&body).unwrap();

    assert_eq!("aa_aa", value.name);
}
