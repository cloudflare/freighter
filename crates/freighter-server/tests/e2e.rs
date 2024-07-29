#![cfg(feature = "test_e2e")]
pub mod common;

use std::collections::HashMap;
use std::env::var;

use anyhow::Result;
use axum::{routing::IntoMakeService, Router, Server};
use deadpool_postgres::Config;
use freighter_api_types::index::request::{Publish, PublishDependency};
use freighter_api_types::index::IndexProvider;
use freighter_auth::pg_backend::PgAuthProvider;
use freighter_auth::AuthProvider;
use freighter_client::Client;
use freighter_fs_index::FsIndexProvider;
use freighter_pg_index::PgIndexProvider;
use freighter_server::ServiceConfig;
use freighter_storage::s3_client::S3StorageProvider;
use hyper::{server::conn::AddrIncoming, Body};
use semver::{Version, VersionReq};
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Clone)]
struct TestServerConfig {
    db: Config,
    server_addr: String,
    bucket_name: String,
    bucket_endpoint_url: String,
    bucket_access_key_id: String,
    bucket_access_key_secret: String,
    auth_required: bool,
}

impl TestServerConfig {
    fn from_env(default_port: u16) -> TestServerConfig {
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
            server_addr: var("SERVER_ADDR").unwrap_or(format!("127.0.0.1:{default_port}")),
            bucket_name: var("BUCKET_NAME").unwrap_or("crates".to_owned()),
            bucket_endpoint_url: var("BUCKET_ENDPOINT")
                .unwrap_or("http://127.0.0.1:9090".to_owned()),
            bucket_access_key_id: var("BUCKET_ACCESS_KEY_ID").unwrap_or("1234567890".to_owned()),
            bucket_access_key_secret: var("BUCKET_ACCESS_KEY_SECRET")
                .unwrap_or("valid-secret".to_owned()),
            auth_required: false,
        }
    }
}

fn server(
    config: &TestServerConfig,
    index_client: impl IndexProvider + Send + Sync + 'static,
    auth_client: impl AuthProvider + Send + Sync + 'static,
) -> Result<Server<AddrIncoming, IntoMakeService<Router<(), Body>>>> {
    let storage_client = S3StorageProvider::new(
        &config.bucket_name,
        &config.bucket_endpoint_url,
        "us-east-1",
        &config.bucket_access_key_id,
        &config.bucket_access_key_secret,
    );

    let service = ServiceConfig {
        address: config.server_addr.parse()?,
        download_endpoint: format!("http://{}/downloads/", config.server_addr),
        api_endpoint: format!("http://{}", config.server_addr.to_owned()),
        metrics_address: "127.0.0.1:9999".parse()?,
        allow_registration: true,
        auth_required: config.auth_required,
        crate_size_limit: 1024 * 1024,
    };

    let router = freighter_server::router(service, index_client, storage_client, auth_client);

    Ok(axum::Server::bind(&config.server_addr.parse()?).serve(router.into_make_service()))
}

#[tokio::test]
async fn e2e_publish_crate_pg() {
    let config = TestServerConfig::from_env(3000);

    type ProviderConfig = <PgIndexProvider as IndexProvider>::Config;
    e2e_publish_crate_in_index(
        PgIndexProvider::new(ProviderConfig {
            index_db: config.db.clone(),
        })
        .unwrap(),
        config,
    )
    .await;
}

#[tokio::test]
async fn e2e_publish_crate_fs() {
    let config = TestServerConfig::from_env(3001);
    let dir = tempfile::tempdir().unwrap();

    type ProviderConfig = <FsIndexProvider as IndexProvider>::Config;
    let index_config = ProviderConfig::Path(dir.path().into());
    e2e_publish_crate_in_index(FsIndexProvider::new(index_config).unwrap(), config).await;
}

#[tokio::test]
async fn e2e_publish_crate_fs_s3() {
    let config = TestServerConfig::from_env(3002);

    type ProviderConfig = <FsIndexProvider as IndexProvider>::Config;
    let index_config = ProviderConfig::S3(freighter_fs_index::StoreConfig {
        name: config.bucket_name.clone(),
        endpoint_url: config.bucket_endpoint_url.clone(),
        region: "us-east-1".into(),
        access_key_id: Some(config.bucket_access_key_id.clone()),
        access_key_secret: Some(config.bucket_access_key_secret.clone()),
    });
    e2e_publish_crate_in_index(FsIndexProvider::new(index_config).unwrap(), config).await;
}

#[tokio::test]
async fn e2e_publish_crate_fs_auth_required() {
    let mut config = TestServerConfig::from_env(3003);
    config.auth_required = true;
    let dir = tempfile::tempdir().unwrap();

    type ProviderConfig = <FsIndexProvider as IndexProvider>::Config;
    let index_config = ProviderConfig::Path(dir.path().into());
    e2e_publish_crate_in_index(FsIndexProvider::new(index_config).unwrap(), config).await;
}

async fn e2e_publish_crate_in_index(
    index_client: impl IndexProvider + Send + Sync + 'static,
    config: TestServerConfig,
) {
    static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _throttle = ONE_AT_A_TIME.lock().await;

    let subscriber = tracing_subscriber::fmt().finish();
    let _guard = subscriber.set_default();

    let server_addr = config.server_addr.clone();

    use rand::distributions::{Alphanumeric, DistString};
    let test_unique_str = Alphanumeric.sample_string(&mut rand::thread_rng(), 12);

    let crate_to_publish = format!("freighter-vegetables-{test_unique_str}");
    let crate_to_publish_2 = format!("freighter-fruits-{test_unique_str}");
    let client_username = format!("kargo-{test_unique_str}");

    type AuthConfig = <PgAuthProvider as AuthProvider>::Config;
    let auth_config = AuthConfig {
        auth_db: config.db.clone(),
    };
    let auth_client = PgAuthProvider::new(auth_config).expect("Failed to initialize auth client");

    let default_token = if config.auth_required {
        Some(auth_client.register(&client_username).await.unwrap())
    } else {
        None
    };

    // 0. Start Freighter
    let server_spawned = tokio::spawn(server(&config, index_client, auth_client).unwrap());

    let mut freighter_client =
        Client::new(&format!("http://{server_addr}/index"), default_token).await;

    if !config.auth_required {
        // 1. Create a user to get a publish token.
        freighter_client.register(&client_username).await.unwrap();
    }

    // 2. Publish a crate!
    let tarball = [1u8; 100];

    freighter_client
        .publish(
            &Publish {
                name: crate_to_publish.clone(),
                vers: Version::new(1, 2, 3),
                deps: vec![PublishDependency {
                    name: "tokio".to_string(),
                    version_req: VersionReq::parse("1.0").unwrap(),
                    features: vec!["net".to_string(), "process".to_string(), "rt".to_string()],
                    optional: false,
                    default_features: false,
                    target: None,
                    kind: Default::default(),
                    registry: Some("https://github.com/rust-lang/crates.io-index".to_string()),
                    explicit_name_in_toml: None,
                }],
                features: HashMap::from_iter([("foo".to_string(), vec!["tokio/fs".to_string()])]),
                authors: vec![],
                description: None,
                documentation: None,
                homepage: None,
                readme: None,
                readme_file: None,
                keywords: vec![],
                categories: vec![],
                license: None,
                license_file: None,
                repository: None,
                badges: None,
                links: None,
            },
            &tarball,
        )
        .await
        .unwrap();

    // 3. Try and publish it again, expect 409 Conflict.
    let publish_res = freighter_client
        .publish(
            &Publish {
                name: crate_to_publish.clone(),
                vers: Version::new(1, 2, 3),
                deps: vec![PublishDependency {
                    name: "tokio".to_string(),
                    version_req: VersionReq::parse("1.0").unwrap(),
                    features: vec!["net".to_string(), "process".to_string(), "rt".to_string()],
                    optional: false,
                    default_features: false,
                    target: None,
                    kind: Default::default(),
                    registry: Some("https://github.com/rust-lang/crates.io-index".to_string()),
                    explicit_name_in_toml: None,
                }],
                features: HashMap::from_iter([("foo".to_string(), vec!["tokio/fs".to_string()])]),
                authors: vec![],
                description: None,
                documentation: None,
                homepage: None,
                readme: None,
                readme_file: None,
                keywords: vec![],
                categories: vec![],
                license: None,
                license_file: None,
                repository: None,
                badges: None,
                links: None,
            },
            &tarball,
        )
        .await
        .unwrap_err();

    assert!(
        matches!(publish_res, freighter_client::Error::Conflict),
        "{:?}",
        publish_res
    );

    // 4. Publish a newer version
    freighter_client
        .publish(
            &Publish {
                name: crate_to_publish_2.clone(),
                vers: Version::new(2, 0, 0),
                deps: vec![PublishDependency {
                    name: crate_to_publish.clone(),
                    version_req: VersionReq::parse("1.2").unwrap(),
                    features: vec!["foo".to_string()],
                    optional: false,
                    default_features: false,
                    target: None,
                    kind: Default::default(),
                    registry: None,
                    explicit_name_in_toml: None,
                }],
                features: HashMap::new(),
                authors: vec![],
                description: None,
                documentation: None,
                homepage: None,
                readme: None,
                readme_file: None,
                keywords: vec![],
                categories: vec![],
                license: None,
                license_file: None,
                repository: None,
                badges: None,
                links: None,
            },
            &tarball,
        )
        .await
        .unwrap();

    // 5. Fetch our crate
    let body = freighter_client
        .download_crate(&crate_to_publish, &Version::new(1, 2, 3))
        .await
        .unwrap();

    // 6. List crates - unsupported

    // 7. Fetch index for crate
    let index = freighter_client
        .fetch_index(&crate_to_publish)
        .await
        .unwrap();

    assert_eq!(index.len(), 1);

    assert_eq!(body, &tarball[..]);

    server_spawned.abort();
}
