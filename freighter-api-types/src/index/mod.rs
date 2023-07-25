#[cfg(any(feature = "client", feature = "server"))]
use serde::{Deserialize, Serialize};

pub mod request;

pub mod response;

#[derive(Debug, Default, Clone)]
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
