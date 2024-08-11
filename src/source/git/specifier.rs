use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{manifest::target::TargetKind, source::DependencySpecifier};

/// The specifier for a Git dependency
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct GitDependencySpecifier {
    /// The repository of the package
    #[serde(
        serialize_with = "crate::util::serialize_gix_url",
        deserialize_with = "crate::util::deserialize_git_like_url"
    )]
    pub repo: gix::Url,
    /// The revision of the package
    pub rev: String,
    /// The target to use for the package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetKind>,
}
impl DependencySpecifier for GitDependencySpecifier {}

impl Display for GitDependencySpecifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}", self.repo, self.rev)
    }
}
