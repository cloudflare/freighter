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
