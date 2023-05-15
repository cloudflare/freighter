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
pub enum DependencyKind {
    #[default]
    Normal,
    Dev,
    Build,
}
