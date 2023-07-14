use freighter_index::CrateVersion;
use semver::Version;

pub fn crate_version(name: &str, version: &str) -> CrateVersion {
    CrateVersion {
        name: name.to_owned(),
        vers: Version::parse(version).unwrap(),
        deps: vec![],
        cksum: "beefcafe".to_owned(),
        features: Default::default(),
        yanked: false,
        links: None,
        v: 2,
        features2: Default::default(),
    }
}

pub fn generate_crate_payload(name: &str, vers: &str, tarball: &[u8]) -> Vec<u8> {
    let json = serde_json::json!({
        "name": name,
        "vers": vers,
        "deps": [],
        "features": {},
        "description": null,
        "documentation": null,
        "homepage": null,
        "readme": null,
        "readme_file": null,
        "license": null,
        "license_file": null,
        "repository": null,
        "badges": null,
        "links": null,
    })
    .to_string();

    // https://github.com/rust-lang/cargo/blob/20df9e40a4d41dd08478549915588395e55efb4c/crates/crates-io/lib.rs#L259
    let payload = {
        let mut payload = Vec::new();
        payload.extend_from_slice(&(json.len() as u32).to_le_bytes());
        payload.extend_from_slice(json.as_bytes());
        payload.extend_from_slice(&(tarball.len() as u32).to_le_bytes());
        payload.extend_from_slice(tarball);
        payload
    };

    payload
}
