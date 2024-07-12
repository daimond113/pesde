use crate::{
    names::{PackageName, PackageNames},
    source::{DependencySpecifiers, PackageRefs},
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Lockfile {
    pub name: PackageName,

    pub specifiers: BTreeMap<PackageNames, BTreeMap<Version, DependencySpecifiers>>,
    pub dependencies: BTreeMap<PackageNames, BTreeMap<Version, LockfileNode>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LockfileNode {
    pub pkg_ref: PackageRefs,
}
