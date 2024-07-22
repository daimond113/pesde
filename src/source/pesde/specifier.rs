use crate::{manifest::TargetKind, names::PackageName, source::DependencySpecifier};
use semver::VersionReq;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct PesdeDependencySpecifier {
    pub name: PackageName,
    pub version: VersionReq,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetKind>,
}
impl DependencySpecifier for PesdeDependencySpecifier {}

impl Display for PesdeDependencySpecifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.name, self.version)
    }
}
