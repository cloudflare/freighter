use deadpool_postgres::tokio_postgres::types::{FromSql, ToSql};
use serde::{Deserialize, Serialize};

pub mod index;

pub mod api;

#[derive(
    Copy,
    Clone,
    Ord,
    PartialOrd,
    Eq,
    PartialEq,
    Debug,
    Default,
    Serialize,
    Deserialize,
    ToSql,
    FromSql,
)]
#[serde(rename_all = "lowercase")]
#[postgres(name = "dependency_kind")]
pub enum DependencyKind {
    #[postgres(name = "normal")]
    #[default]
    Normal,
    #[postgres(name = "dev")]
    Dev,
    #[postgres(name = "build")]
    Build,
}
