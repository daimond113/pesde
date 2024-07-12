use std::{collections::BTreeMap, fmt::Debug, path::Path};

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::Project;

pub mod pesde;

pub trait DependencySpecifier: Debug {
    fn alias(&self) -> &str;
    fn set_alias(&mut self, alias: String);
}

pub trait PackageRef: Debug {}

pub(crate) fn hash<S: std::hash::Hash>(struc: &S) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    let mut hasher = DefaultHasher::new();
    struc.hash(&mut hasher);
    hasher.finish().to_string()
}

pub trait PackageSource: Debug {
    type Ref: PackageRef;
    type Specifier: DependencySpecifier;
    type RefreshError: std::error::Error;
    type ResolveError: std::error::Error;
    type DownloadError: std::error::Error;

    fn refresh(&self, _project: &Project) -> Result<(), Self::RefreshError> {
        Ok(())
    }

    fn resolve(
        &self,
        specifier: &Self::Specifier,
        project: &Project,
    ) -> Result<BTreeMap<Version, Self::Ref>, Self::ResolveError>;

    fn download(
        &self,
        pkg_ref: &Self::Ref,
        destination: &Path,
        project: &Project,
    ) -> Result<(), Self::DownloadError>;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum DependencySpecifiers {
    Pesde(pesde::PesdeDependencySpecifier),
}

impl DependencySpecifiers {
    pub fn alias(&self) -> &str {
        match self {
            DependencySpecifiers::Pesde(spec) => spec.alias(),
        }
    }

    pub fn set_alias(&mut self, alias: String) {
        match self {
            DependencySpecifiers::Pesde(spec) => spec.set_alias(alias),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum PackageRefs {
    Pesde(pesde::PesdePackageRef),
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub enum PackageSources {
    Pesde(pesde::PesdePackageSource),
}
