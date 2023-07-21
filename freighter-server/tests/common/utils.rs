use freighter_index::{CrateVersion, PublishDependency};
use semver::{Version, VersionReq};
use std::str::FromStr;

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

pub fn generate_crate_payload(
    name: &str,
    vers: &str,
    tarball: &[u8],
    deps: &[(&str, &str, Option<&str>)],
) -> Vec<u8> {
    let deps: Vec<_> = deps
        .iter()
        .map(|(name, req, registry)| PublishDependency {
            name: name.to_string(),
            version_req: VersionReq::from_str(req).unwrap(),
            features: vec![],
            optional: false,
            default_features: false,
            target: None,
            kind: Default::default(),
            registry: registry.map(|x| x.to_string()),
            explicit_name_in_toml: None,
        })
        .collect();

    let json = serde_json::json!({
        "name": name,
        "vers": vers,
        "deps": deps,
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
