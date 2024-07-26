use std::collections::{BTreeMap, BTreeSet};

use relative_path::RelativePathBuf;
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{
    manifest::{overrides::OverrideKey, target::Target},
    names::{PackageName, PackageNames},
    source::{DependencySpecifiers, VersionId},
};

pub mod overrides;
pub mod target;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Manifest {
    pub name: PackageName,
    pub version: Version,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    pub target: Target,
    #[serde(default)]
    pub private: bool,
    #[serde(default, skip_serializing)]
    pub scripts: BTreeMap<String, RelativePathBuf>,
    #[serde(
        default,
        serialize_with = "crate::util::serialize_gix_url_map",
        deserialize_with = "crate::util::deserialize_gix_url_map"
    )]
    pub indices: BTreeMap<String, gix::Url>,
    #[cfg(feature = "wally-compat")]
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        serialize_with = "crate::util::serialize_gix_url_map",
        deserialize_with = "crate::util::deserialize_gix_url_map"
    )]
    pub wally_indices: BTreeMap<String, gix::Url>,
    #[serde(default, skip_serializing)]
    pub overrides: BTreeMap<OverrideKey, DependencySpecifiers>,
    #[serde(default)]
    pub includes: BTreeSet<String>,
    #[cfg(feature = "patches")]
    #[serde(default, skip_serializing)]
    pub patches: BTreeMap<PackageNames, BTreeMap<VersionId, RelativePathBuf>>,
    #[serde(default, skip_serializing)]
    pub pesde_version: Option<Version>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, DependencySpecifiers>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub peer_dependencies: BTreeMap<String, DependencySpecifiers>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dev_dependencies: BTreeMap<String, DependencySpecifiers>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    Standard,
    Dev,
    Peer,
}

impl Manifest {
    pub fn all_dependencies(
        &self,
    ) -> Result<
        BTreeMap<String, (DependencySpecifiers, DependencyType)>,
        errors::AllDependenciesError,
    > {
        let mut all_deps = BTreeMap::new();

        for (deps, ty) in [
            (&self.dependencies, DependencyType::Standard),
            (&self.peer_dependencies, DependencyType::Peer),
            (&self.dev_dependencies, DependencyType::Dev),
        ] {
            for (alias, spec) in deps {
                if all_deps.insert(alias.clone(), (spec.clone(), ty)).is_some() {
                    return Err(errors::AllDependenciesError::AliasConflict(alias.clone()));
                }
            }
        }

        Ok(all_deps)
    }
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum AllDependenciesError {
        #[error("another specifier is already using the alias {0}")]
        AliasConflict(String),
    }
}
