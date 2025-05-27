// Run with:
// cargo t -F filesystem-index-backend,yes-auth-backend
//
// This test doesn't set up any auth, so it won't work if proper auth is enabled
#[cfg(feature = "yes-auth-backend")]
#[cfg(feature = "filesystem-index-backend")]
#[cfg(not(feature = "filesystem-auth-backend"))]
#[cfg(not(feature = "cloudflare-auth-backend"))]
#[tokio::test]
async fn cargo_client_yes_auth_backend() {
    let _ = tracing_subscriber::fmt::fmt().with_test_writer().try_init();

    let listener = freighter::start_listening(freighter::cli::FreighterArgs {
        config: "../../configs/local.s3-based.yaml".into(),
    })
    .await
    .unwrap();

    let server_handle = tokio::task::spawn(listener).abort_handle();

    tokio::task::spawn_blocking(move || {
        test_cargo_publish();
        server_handle.abort()
    })
    .await
    .unwrap();
}

fn test_cargo_publish() {
    let tempdir = tempfile::tempdir().unwrap();
    let path = tempdir.path();

    let random_id: u128 = rand::random();
    let test_crate1_name = format!("test_crate1_{random_id:x}");

    let cargo_dir = path.join(".cargo");
    std::fs::create_dir(&cargo_dir).unwrap();
    let cargo_config = cargo_dir.join("config.toml");
    std::fs::write(
        &cargo_config,
        format!(
            r#"
[registries.test_registry]
index = "sparse+http://127.0.0.1:8080/index/"
    "#
        ),
    )
    .unwrap();

    let test_crate1 = path.join("test_crate1");
    let src = test_crate1.join("src");
    std::fs::create_dir_all(&test_crate1.join("src")).unwrap();

    std::fs::write(
        &src.join("lib.rs"),
        format!("pub const ID_{random_id}: bool = true;"),
    )
    .unwrap();
    std::fs::write(
        &test_crate1.join("Cargo.toml"),
        format!(
            r#"
[package]
name = "{test_crate1_name}"
edition = "2024"
version = "1.0.0"
description = "test"
license = "MIT"
publish = ["test_registry"]
"#
        ),
    )
    .unwrap();

    let res = std::process::Command::new("cargo")
        .arg("publish")
        .current_dir(&test_crate1)
        .env("CARGO_REGISTRIES_TEST_REGISTRY_TOKEN", "ok")
        .status()
        .unwrap();
    assert!(res.success());

    let test_crate2 = path.join("test_crate2");
    let src = test_crate2.join("src");
    std::fs::create_dir_all(&test_crate2.join("src")).unwrap();

    std::fs::write(
        &src.join("lib.rs"),
        format!("pub use {test_crate1_name}::ID_{random_id};"),
    )
    .unwrap();
    std::fs::write(
        &test_crate2.join("Cargo.toml"),
        format!(
            r#"
[package]
name = "test_crate2"
edition = "2024"
version = "1.0.0"
description = "test"
license = "MIT"
publish = ["test_registry"]

[dependencies]
{test_crate1_name} = {{ version = "1", registry = "test_registry" }}
    "#
        ),
    )
    .unwrap();

    let res = std::process::Command::new("cargo")
        .current_dir(&test_crate2)
        .arg("b")
        .status()
        .unwrap();
    assert!(res.success());
}
