use crate::{
    manifest::{target::TargetKind, DependencyType},
    source::{pesde, specifiers::DependencySpecifiers, traits::PackageRef, PackageSources},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// All possible package references
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case", tag = "ref_ty")]
pub enum PackageRefs {
    /// A pesde package reference
    Pesde(pesde::pkg_ref::PesdePackageRef),
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

    fn source(&self) -> PackageSources {
        match self {
            PackageRefs::Pesde(pkg_ref) => pkg_ref.source(),
        }
    }
}
