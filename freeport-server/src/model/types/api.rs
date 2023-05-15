use crate::model::types::DependencyKind;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
pub struct PublishOperationInfo {
    /// Optional object of warnings to display to the user.
    pub warnings: Option<PublishOperationWarnings>,
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct PublishOperationWarnings {
    /// Array of strings of categories that are invalid and ignored.
    pub invalid_categories: Vec<String>,
    /// Array of strings of badge names that are invalid and ignored.
    pub invalid_badges: Vec<String>,
    /// Array of strings of arbitrary warnings to display to the user.
    pub other: Vec<String>,
}
