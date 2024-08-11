use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    manifest::{target::TargetKind, DependencyType},
    source::{git::GitPackageSource, DependencySpecifiers, PackageRef, PackageSources},
};

/// A Git package reference
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct GitPackageRef {
    /// The repository of the package
    #[serde(
        serialize_with = "crate::util::serialize_gix_url",
        deserialize_with = "crate::util::deserialize_gix_url"
    )]
    pub repo: gix::Url,
    /// The revision of the package
    pub rev: String,
    /// The dependencies of the package
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, (DependencySpecifiers, DependencyType)>,
    /// Whether this package uses the new structure
    pub new_structure: bool,
    /// The target of the package
    pub target: TargetKind,
}
impl PackageRef for GitPackageRef {
    fn dependencies(&self) -> &BTreeMap<String, (DependencySpecifiers, DependencyType)> {
        &self.dependencies
    }

    fn use_new_structure(&self) -> bool {
        self.new_structure
    }

    fn target_kind(&self) -> TargetKind {
        self.target
    }

    fn source(&self) -> PackageSources {
        PackageSources::Git(GitPackageSource::new(self.repo.clone()))
    }
}
