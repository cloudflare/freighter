use serde::{Deserialize, Serialize};

pub mod index;

pub mod api;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(
    feature = "sql",
    derive(postgres_types::ToSql, postgres_types::FromSql)
)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "sql", postgres(name = "dependency_kind"))]
pub enum DependencyKind {
    #[cfg_attr(feature = "sql", postgres(name = "normal"))]
    #[default]
    Normal,
    #[cfg_attr(feature = "sql", postgres(name = "dev"))]
    Dev,
    #[cfg_attr(feature = "sql", postgres(name = "build"))]
    Build,
}
