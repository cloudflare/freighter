use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
#[cfg_attr(
    feature = "postgresql-backend",
    derive(postgres_types::ToSql, postgres_types::FromSql)
)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "postgresql-backend", postgres(name = "dependency_kind"))]
pub enum DependencyKind {
    #[cfg_attr(feature = "postgresql-backend", postgres(name = "normal"))]
    #[default]
    Normal,
    #[cfg_attr(feature = "postgresql-backend", postgres(name = "dev"))]
    Dev,
    #[cfg_attr(feature = "postgresql-backend", postgres(name = "build"))]
    Build,
}

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

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct AuthForm {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct SearchQuery {
    /// The search query string.
    pub q: String,
    /// Number of results, default 10, max 100.
    pub per_page: Option<usize>,
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct SearchResults {
    /// Array of results.
    pub crates: Vec<SearchResultsEntry>,
    pub meta: SearchResultsMeta,
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct SearchResultsMeta {
    /// Total number of results available on the server.
    pub total: usize,
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct SearchResultsEntry {
    /// Name of the crate.
    pub name: String,
    /// The highest version available.
    pub max_version: Version,
    /// Textual description of the crate.
    pub description: String,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Publish {
    /// The name of the package.
    pub name: String,
    /// The version of the package being published.
    pub vers: Version,
    /// Array of direct dependencies of the package.
    pub deps: Vec<PublishDependency>,
    /// Set of features defined for the package.
    ///
    /// Each feature maps to an array of features or dependencies it enables.
    /// Cargo does not impose limitations on feature names, but crates.io requires alphanumeric
    /// ASCII, `_` or `-` characters.
    pub features: HashMap<String, Vec<String>>,
    /// List of strings of the authors.
    ///
    /// May be empty.
    #[serde(default)]
    pub authors: Vec<String>,
    /// Description field from the manifest.
    ///
    /// May be null. crates.io requires at least some content.
    pub description: Option<String>,
    /// String of the URL to the website for this package's documentation.
    ///
    /// May be null.
    pub documentation: Option<String>,
    /// String of the URL to the website for this package's home page.
    ///
    /// May be null.
    pub homepage: Option<String>,
    /// String of the content of the README file.
    ///
    /// May be null.
    pub readme: Option<String>,
    /// String of a relative path to a README file in the crate.
    ///
    /// May be null.
    pub readme_file: Option<String>,
    /// Array of strings of keywords for the package.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Array of strings of categories for the package.
    #[serde(default)]
    pub categories: Vec<String>,
    /// String of the license for the package.
    ///
    /// May be null. crates.io requires either `license` or `license_file` to be set.
    pub license: Option<String>,
    /// String of a relative path to a license file in the crate.
    ///
    /// May be null.
    pub license_file: Option<String>,
    /// String of the URL to the website for the source repository of this package.
    ///
    /// May be null.
    pub repository: Option<String>,
    /// Optional object of "status" badges.
    ///
    /// Each value is an object of arbitrary string to string mappings.
    /// crates.io has special interpretation of the format of the badges.
    pub badges: Option<HashMap<String, HashMap<String, String>>>,
    /// The `links` string value from the package's manifest, or null if not specified.
    ///
    /// This field is optional and defaults to null.
    pub links: Option<String>,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct PublishDependency {
    /// Name of the dependency.
    ///
    /// If the dependency is renamed from the original package name, this is the original name.
    /// The new package name is stored in the [`explicit_name_in_toml`] field.
    pub name: String,
    /// The semver requirement for this dependency.
    pub version_req: VersionReq,
    /// Array of features (as strings) enabled for this dependency.
    pub features: Vec<String>,
    /// Boolean of whether or not this is an optional dependency.
    pub optional: bool,
    /// Boolean of whether or not default features are enabled.
    pub default_features: bool,
    /// The target platform for the dependency.
    ///
    /// Null if not a target dependency. Otherwise, a string such as "cfg(windows)".
    pub target: Option<String>,
    /// The dependency kind.
    ///
    /// "dev", "build", or "normal".
    pub kind: DependencyKind,
    /// The URL of the index of the registry where this dependency is from as a string.
    ///
    /// If not specified or null, it is assumed the dependency is in the current registry.
    pub registry: Option<String>,
    /// If the dependency is renamed, this is a string of the new package name.
    ///
    /// If not specified or null, this dependency is not renamed.
    pub explicit_name_in_toml: Option<String>,
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CompletedPublication {
    /// Optional object of warnings to display to the user.
    pub warnings: Option<CompletedPublicationWarnings>,
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct CompletedPublicationWarnings {
    /// Array of strings of categories that are invalid and ignored.
    pub invalid_categories: Vec<String>,
    /// Array of strings of badge names that are invalid and ignored.
    pub invalid_badges: Vec<String>,
    /// Array of strings of arbitrary warnings to display to the user.
    pub other: Vec<String>,
}
