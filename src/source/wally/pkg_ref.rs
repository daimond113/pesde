use std::collections::BTreeMap;

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{
    manifest::{target::TargetKind, DependencyType},
    names::wally::WallyPackageName,
    source::{wally::WallyPackageSource, DependencySpecifiers, PackageRef, PackageSources},
};

/// A Wally package reference
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct WallyPackageRef {
    /// The name of the package
    #[serde(rename = "wally")]
    pub name: WallyPackageName,
    /// The version of the package
    pub version: Version,
    /// The index of the package
    #[serde(
        serialize_with = "crate::util::serialize_gix_url",
        deserialize_with = "crate::util::deserialize_gix_url"
    )]
    pub index_url: gix::Url,
    /// The dependencies of the package
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, (DependencySpecifiers, DependencyType)>,
}
impl PackageRef for WallyPackageRef {
    fn dependencies(&self) -> &BTreeMap<String, (DependencySpecifiers, DependencyType)> {
        &self.dependencies
    }

    fn use_new_structure(&self) -> bool {
        false
    }

    fn target_kind(&self) -> TargetKind {
        TargetKind::Roblox
    }

    fn source(&self) -> PackageSources {
        PackageSources::Wally(WallyPackageSource::new(self.index_url.clone()))
    }
}

impl Ord for WallyPackageRef {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.version.cmp(&other.version)
    }
}

impl PartialOrd for WallyPackageRef {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
