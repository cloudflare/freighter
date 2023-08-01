use crate::index::request::PublishDependency;
use crate::index::response::CrateVersion;
#[cfg(any(feature = "client", feature = "server"))]
use serde::{Deserialize, Serialize};

pub mod request;

pub mod response;

#[derive(Debug, Default, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(
    any(feature = "server", feature = "client"),
    derive(Serialize, Deserialize),
    serde(rename_all = "lowercase")
)]
#[cfg_attr(
    feature = "postgres",
    derive(postgres_types::ToSql, postgres_types::FromSql),
    postgres(name = "dependency_kind")
)]
pub enum DependencyKind {
    #[cfg_attr(feature = "postgres", postgres(name = "normal"))]
    #[default]
    Normal,
    #[cfg_attr(feature = "postgres", postgres(name = "dev"))]
    Dev,
    #[cfg_attr(feature = "postgres", postgres(name = "build"))]
    Build,
}

impl From<response::CrateVersion> for request::Publish {
    fn from(value: CrateVersion) -> Self {
        Self {
            name: value.name,
            vers: value.vers,
            deps: value
                .deps
                .into_iter()
                .map(|x| PublishDependency {
                    name: x.name,
                    version_req: x.req,
                    features: x.features,
                    optional: x.optional,
                    default_features: x.default_features,
                    target: x.target,
                    kind: x.kind,
                    registry: x.registry,
                    explicit_name_in_toml: x.package,
                })
                .collect(),
            features: value.features,
            /// Note: We do not carry over authors since its not in index
            authors: Vec::new(),
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
        }
    }
}
