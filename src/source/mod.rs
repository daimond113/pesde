use std::{collections::BTreeMap, fmt::Debug};

use crate::{
    manifest::target::{Target, TargetKind},
    names::PackageNames,
    source::{
        fs::PackageFS, refs::PackageRefs, specifiers::DependencySpecifiers, traits::*,
        version_id::VersionId,
    },
    Project,
};

/// Packages' filesystems
pub mod fs;
/// The pesde package source
pub mod pesde;
/// Package references
pub mod refs;
/// Dependency specifiers
pub mod specifiers;
/// Traits for sources and packages
pub mod traits;
/// Version IDs
pub mod version_id;

/// The result of resolving a package
pub type ResolveResult<Ref> = (PackageNames, BTreeMap<VersionId, Ref>);

/// All possible package sources
#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum PackageSources {
    /// A pesde package source
    Pesde(pesde::PesdePackageSource),
}

impl PackageSource for PackageSources {
    type Specifier = DependencySpecifiers;
    type Ref = PackageRefs;
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
        project: &Project,
        reqwest: &reqwest::blocking::Client,
    ) -> Result<(PackageFS, Target), Self::DownloadError> {
        match (self, pkg_ref) {
            (PackageSources::Pesde(source), PackageRefs::Pesde(pkg_ref)) => source
                .download(pkg_ref, project, reqwest)
                .map_err(Into::into),

            _ => Err(errors::DownloadError::Mismatch),
        }
    }
}

/// Errors that can occur when interacting with a package source
pub mod errors {
    use thiserror::Error;

    /// Errors that occur when refreshing a package source
    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum RefreshError {
        /// The pesde package source failed to refresh
        #[error("error refreshing pesde package source")]
        Pesde(#[from] crate::source::pesde::errors::RefreshError),
    }

    /// Errors that can occur when resolving a package
    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum ResolveError {
        /// The dependency specifier does not match the source (if using the CLI, this is a bug - file an issue)
        #[error("mismatched dependency specifier for source")]
        Mismatch,

        /// The pesde package source failed to resolve
        #[error("error resolving pesde package")]
        Pesde(#[from] crate::source::pesde::errors::ResolveError),
    }

    /// Errors that can occur when downloading a package
    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum DownloadError {
        /// The package ref does not match the source (if using the CLI, this is a bug - file an issue)
        #[error("mismatched package ref for source")]
        Mismatch,

        /// The pesde package source failed to download
        #[error("error downloading pesde package")]
        Pesde(#[from] crate::source::pesde::errors::DownloadError),
    }
}
