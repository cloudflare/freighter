#![cfg(feature = "test_e2e")]
pub mod common;

use std::env::var;

use anyhow::{Context, Result};
use axum::{routing::IntoMakeService, Router, Server};
use deadpool_postgres::Config;
use freighter_auth::pg_backend::PgAuthProvider;
use freighter_index::postgres_client::PgIndexProvider;
use freighter_server::ServiceConfig;
use freighter_storage::s3_client::S3StorageProvider;
use hyper::{header::AUTHORIZATION, server::conn::AddrIncoming, Body, StatusCode};

use crate::common::utils::generate_crate_payload;

#[derive(Clone)]
struct TestServerConfig {
    db: Config,
    server_addr: String,
    bucket_name: String,
    bucket_endpoint_url: String,
    bucket_access_key_id: String,
    bucket_access_key_secret: String,
}

impl TestServerConfig {
    fn from_env() -> TestServerConfig {
        Self {
            db: Config {
                user: Some(var("POSTGRES_USER").unwrap_or("freighter".to_owned())),
                password: Some(
                    var("POSTGRES_PASSWORD").unwrap_or("crates-crates-crates".to_owned()),
                ),
                dbname: Some(var("POSTGRES_DBNAME").unwrap_or("freighter".to_owned())),
                host: Some(var("POSTGRES_HOST").unwrap_or("localhost".to_owned())),
                port: Some(
                    var("POSTGRES_PORT")
                        .map(|p| p.parse::<u16>().unwrap())
                        .unwrap_or(5432),
                ),
                ..Default::default()
            },
            server_addr: var("SERVER_ADDR").unwrap_or("127.0.0.1:3000".to_owned()),
            bucket_name: var("BUCKET_NAME").unwrap_or("crates".to_owned()),
            bucket_endpoint_url: var("BUCKET_ENDPOINT")
                .unwrap_or("http://127.0.0.1:9090".to_owned()),
            bucket_access_key_id: var("BUCKET_ACCESS_KEY_ID").unwrap_or("1234567890".to_owned()),
            bucket_access_key_secret: var("BUCKET_ACCESS_KEY_SECRET")
                .unwrap_or("valid-secret".to_owned()),
        }
    }
}

fn server(
    config: &TestServerConfig,
) -> Result<Server<AddrIncoming, IntoMakeService<Router<(), Body>>>> {
    let index_client =
        PgIndexProvider::new(config.db.clone()).context("Failed to construct index client")?;
    let storage_client = S3StorageProvider::new(
        &config.bucket_name,
        &config.bucket_endpoint_url,
        "us-east-1",
        &config.bucket_access_key_id,
        &config.bucket_access_key_secret,
    );
    let auth_client =
        PgAuthProvider::new(config.db.clone()).context("Failed to initialize auth client")?;

    let service = ServiceConfig {
        address: config.server_addr.parse()?,
        download_endpoint: format!("{}/downloads/{{crate}}/{{version}}", config.server_addr),
        api_endpoint: config.server_addr.to_owned(),
        metrics_address: "127.0.0.1:9999".parse()?,
    };

    let router = freighter_server::router(service, index_client, storage_client, auth_client);

    Ok(axum::Server::bind(&config.server_addr.parse()?).serve(router.into_make_service()))
}

#[tokio::test]
async fn e2e_publish_crate() {
    let config = TestServerConfig::from_env();
    let server_addr = config.server_addr.clone();

    const CRATE_TO_PUBLISH: &str = "freighter-vegetables";

    // 0. Start Freighter
    {
        let server = server(&config).unwrap();
        tokio::spawn(server);
    }

    // 1. Create a user to get a publish token.
    let client = reqwest::Client::new();

    let token = client
        .post(format!("http://{server_addr}/api/v1/crates/account"))
        .form(&[("username", "kargo"), ("password", "krab")])
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    // 2. Publish a crate!
    let tarball = [1u8; 100];
    let publish_res = client
        .put(format!("http://{server_addr}/api/v1/crates/new"))
        .header(AUTHORIZATION, token.clone())
        .body(generate_crate_payload(CRATE_TO_PUBLISH, "1.2.3", &tarball))
        .send()
        .await
        .unwrap();

    assert_eq!(publish_res.status(), StatusCode::OK);

    // 3. Try and publish it again, expect 409 Conflict.
    let publish_res = client
        .put(format!("http://{server_addr}/api/v1/crates/new"))
        .header(AUTHORIZATION, token.clone())
        .body(generate_crate_payload(CRATE_TO_PUBLISH, "1.2.3", &tarball))
        .send()
        .await
        .unwrap();

    assert_eq!(publish_res.status(), StatusCode::CONFLICT);

    // 4. Publish a newer version
    let publish_res = client
        .put(format!("http://{server_addr}/api/v1/crates/new"))
        .header(AUTHORIZATION, token.clone())
        .body(generate_crate_payload(
            CRATE_TO_PUBLISH,
            "2.0.0",
            &[2u8; 100],
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(publish_res.status(), StatusCode::OK);

    // 5. Fetch our crate
    let crate_res = client
        .get(format!(
            "http://{server_addr}/downloads/{CRATE_TO_PUBLISH}/1.2.3"
        ))
        .send()
        .await
        .unwrap();

    assert_eq!(crate_res.status(), StatusCode::OK);

    let body = crate_res.bytes().await.unwrap();
    assert_eq!(body, &tarball[..])
}
