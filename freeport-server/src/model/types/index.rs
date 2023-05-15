use crate::model::types::DependencyKind;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct CrateVersion {
    /// The name of the package.
    ///
    /// This must only contain alphanumeric, `-`, or `_` characters.
    pub name: String,
    /// The version of the package this row is describing.
    ///
    /// This must be a valid version number according to the Semantic Versioning 2.0.0 spec at
    /// https://semver.org/.
    pub vers: Version,
    /// Array of direct dependencies of the package.
    pub deps: Vec<Dependency>,
    /// A SHA256 checksum of the `.crate` file.
    pub cksum: String,
    /// Set of features defined for the package.
    ///
    /// Each feature maps to an array of features or dependencies it enables.
    pub features: HashMap<String, Vec<String>>,
    /// Boolean of whether or not this version has been yanked.
    pub yanked: bool,
    /// The `links` string value from the package's manifest, or null if not specified.
    ///
    /// This field is optional and defaults to null.
    pub links: Option<String>,
    /// An unsigned 32-bit integer value indicating the schema version of this entry.
    ///
    /// If this not specified, it should be interpreted as the default of 1.
    ///
    /// Cargo (starting with version 1.51) will ignore versions it does not recognize.
    /// This provides a method to safely introduce changes to index entries and allow older
    /// versions of cargo to ignore newer entries it doesn't understand. Versions older than 1.51
    /// ignore this field, and thus may misinterpret the meaning of the index entry.
    ///
    /// The current values are:
    ///
    /// * 1: The schema as documented here, not including newer additions.
    ///      This is honored in Rust version 1.51 and newer.
    /// * 2: The addition of the `features2` field.
    ///      This is honored in Rust version 1.60 and newer.
    pub v: u32,
    /// This optional field contains features with new, extended syntax.
    ///
    /// Specifically, namespaced features (`dep:`) and weak dependencies (`pkg?/feat`).
    ///
    /// This is separated from `features` because versions older than 1.19 will fail to load due to
    /// not being able to parse the new syntax, even with a `Cargo.lock` file.
    ///
    /// Cargo will merge any values listed here with the "features" field.
    ///
    /// If this field is included, the "v" field should be set to at least 2.
    ///
    /// Registries are not required to use this field for extended feature syntax, they are allowed
    /// to include those in the "features" field. Using this is only necessary if the registry
    /// wants to support cargo versions older than 1.19, which in practice is only crates.io since
    /// those older versions do not support other registries.
    pub features2: HashMap<String, Vec<String>>,
}

#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Dependency {
    /// Name of the dependency.
    ///
    /// If the dependency is renamed from the original package name, this is the new name.
    /// The original package name is stored in the `package` field.
    pub name: String,
    /// The SemVer requirement for this dependency.
    ///
    /// This must be a valid version requirement defined at
    /// https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html.
    pub req: VersionReq,
    /// Array of features (as strings) enabled for this dependency.
    pub features: Vec<String>,
    /// Boolean of whether or not this is an optional dependency.
    pub optional: bool,
    /// Boolean of whether or not default features are enabled.
    pub default_features: bool,
    /// The target platform for the dependency.
    ///
    /// null if not a target dependency. Otherwise, a string such as "cfg(windows)".
    pub target: Option<String>,
    /// The dependency kind.
    ///
    /// "dev", "build", or "normal".
    /// Note: this is a required field, but a small number of entries exist in the crates.io index
    /// with either a missing or null kind` field due to implementation bugs.
    pub kind: DependencyKind,
    /// The URL of the index of the registry where this dependency is from as a string.
    ///
    /// If not specified or null, it is assumed the dependency is in the current registry.
    pub registry: Option<String>,
    /// If the dependency is renamed, this is a string of the actual package name.
    ///
    /// If not specified or null, this dependency is not
    /// renamed.
    pub package: Option<String>,
}
