use crate::{
    manifest::{
        target::{Target, TargetKind},
        DependencyType,
    },
    source::{DependencySpecifiers, PackageFS, PackageSources, ResolveResult},
    Project,
};
use std::{
    collections::BTreeMap,
    fmt::{Debug, Display},
};

pub trait DependencySpecifier: Debug + Display {}

pub trait PackageRef: Debug {
    fn dependencies(&self) -> &BTreeMap<String, (DependencySpecifiers, DependencyType)>;
    fn use_new_structure(&self) -> bool;
    fn target_kind(&self) -> TargetKind;
    fn source(&self) -> PackageSources;
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
        project: &Project,
        reqwest: &reqwest::blocking::Client,
    ) -> Result<(PackageFS, Target), Self::DownloadError>;
}
