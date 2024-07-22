use crate::{
    manifest::{DependencyType, Target, TargetKind},
    names::PackageNames,
    Project,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{
    collections::BTreeMap,
    fmt::{Debug, Display},
    path::Path,
    str::FromStr,
};

pub mod pesde;

pub(crate) fn hash<S: std::hash::Hash>(struc: &S) -> String {
    use std::{collections::hash_map::DefaultHasher, hash::Hasher};

    let mut hasher = DefaultHasher::new();
    struc.hash(&mut hasher);
    hasher.finish().to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum DependencySpecifiers {
    Pesde(pesde::specifier::PesdeDependencySpecifier),
}
pub trait DependencySpecifier: Debug + Display {}
impl DependencySpecifier for DependencySpecifiers {}

impl Display for DependencySpecifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencySpecifiers::Pesde(specifier) => write!(f, "{specifier}"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case", tag = "ref_ty")]
pub enum PackageRefs {
    Pesde(pesde::pkg_ref::PesdePackageRef),
}
pub trait PackageRef: Debug {
    fn dependencies(&self) -> &BTreeMap<String, (DependencySpecifiers, DependencyType)>;
    fn use_new_structure(&self) -> bool;
    fn target_kind(&self) -> TargetKind;
}
impl PackageRef for PackageRefs {
    fn dependencies(&self) -> &BTreeMap<String, (DependencySpecifiers, DependencyType)> {
        match self {
            PackageRefs::Pesde(pkg_ref) => pkg_ref.dependencies(),
        }
    }

    fn use_new_structure(&self) -> bool {
        match self {
            PackageRefs::Pesde(pkg_ref) => pkg_ref.use_new_structure(),
        }
    }

    fn target_kind(&self) -> TargetKind {
        match self {
            PackageRefs::Pesde(pkg_ref) => pkg_ref.target_kind(),
        }
    }
}

#[derive(
    Debug, SerializeDisplay, DeserializeFromStr, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct VersionId(Version, TargetKind);

impl VersionId {
    pub fn version(&self) -> &Version {
        &self.0
    }

    pub fn target(&self) -> &TargetKind {
        &self.1
    }
}

impl Display for VersionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.0, self.1)
    }
}

impl FromStr for VersionId {
    type Err = errors::VersionIdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((version, target)) = s.split_once(' ') else {
            return Err(errors::VersionIdParseError::Malformed(s.to_string()));
        };

        let version = version.parse()?;
        let target = target.parse()?;

        Ok(VersionId(version, target))
    }
}

pub type ResolveResult<Ref> = (PackageNames, BTreeMap<VersionId, Ref>);

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum PackageSources {
    Pesde(pesde::PesdePackageSource),
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
        project_target: TargetKind,
    ) -> Result<ResolveResult<Self::Ref>, Self::ResolveError>;

    fn download(
        &self,
        pkg_ref: &Self::Ref,
        destination: &Path,
        project: &Project,
        reqwest: &reqwest::blocking::Client,
    ) -> Result<Target, Self::DownloadError>;
}
impl PackageSource for PackageSources {
    type Ref = PackageRefs;
    type Specifier = DependencySpecifiers;
    type RefreshError = errors::RefreshError;
    type ResolveError = errors::ResolveError;
    type DownloadError = errors::DownloadError;

    fn refresh(&self, project: &Project) -> Result<(), Self::RefreshError> {
        match self {
            PackageSources::Pesde(source) => source.refresh(project).map_err(Into::into),
        }
    }

    fn resolve(
        &self,
        specifier: &Self::Specifier,
        project: &Project,
        project_target: TargetKind,
    ) -> Result<ResolveResult<Self::Ref>, Self::ResolveError> {
        match (self, specifier) {
            (PackageSources::Pesde(source), DependencySpecifiers::Pesde(specifier)) => source
                .resolve(specifier, project, project_target)
                .map(|(name, results)| {
                    (
                        name,
                        results
                            .into_iter()
                            .map(|(version, pkg_ref)| (version, PackageRefs::Pesde(pkg_ref)))
                            .collect(),
                    )
                })
                .map_err(Into::into),

            _ => Err(errors::ResolveError::Mismatch),
        }
    }

    fn download(
        &self,
        pkg_ref: &Self::Ref,
        destination: &Path,
        project: &Project,
        reqwest: &reqwest::blocking::Client,
    ) -> Result<Target, Self::DownloadError> {
        match (self, pkg_ref) {
            (PackageSources::Pesde(source), PackageRefs::Pesde(pkg_ref)) => source
                .download(pkg_ref, destination, project, reqwest)
                .map_err(Into::into),

            _ => Err(errors::DownloadError::Mismatch),
        }
    }
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum RefreshError {
        #[error("error refreshing pesde package source")]
        Pesde(#[from] crate::source::pesde::errors::RefreshError),
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum ResolveError {
        #[error("mismatched dependency specifier for source")]
        Mismatch,

        #[error("error resolving pesde package")]
        Pesde(#[from] crate::source::pesde::errors::ResolveError),
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum DownloadError {
        #[error("mismatched package ref for source")]
        Mismatch,

        #[error("error downloading pesde package")]
        Pesde(#[from] crate::source::pesde::errors::DownloadError),
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum VersionIdParseError {
        #[error("malformed entry key {0}")]
        Malformed(String),

        #[error("malformed version")]
        Version(#[from] semver::Error),

        #[error("malformed target")]
        Target(#[from] crate::manifest::errors::TargetKindFromStr),
    }
}
