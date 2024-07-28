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

pub mod fs;
pub mod pesde;
pub mod refs;
pub mod specifiers;
pub mod traits;
pub mod version_id;

pub type ResolveResult<Ref> = (PackageNames, BTreeMap<VersionId, Ref>);

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum PackageSources {
    Pesde(pesde::PesdePackageSource),
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
}
