use crate::{
    manifest::{DependencyType, OverrideKey},
    names::{PackageName, PackageNames},
    source::{DependencySpecifiers, PackageRefs},
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::{btree_map::Entry, BTreeMap};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependencyGraphNode {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct: Option<(String, DependencySpecifiers)>,
    pub pkg_ref: PackageRefs,
    pub dependencies: BTreeMap<PackageNames, (Version, String)>,
    pub ty: DependencyType,
}

pub type DependencyGraph = BTreeMap<PackageNames, BTreeMap<Version, DependencyGraphNode>>;

pub fn insert_node(
    graph: &mut DependencyGraph,
    name: PackageNames,
    version: Version,
    node: DependencyGraphNode,
) {
    match graph
        .entry(name.clone())
        .or_default()
        .entry(version.clone())
    {
        Entry::Vacant(entry) => {
            entry.insert(node);
        }
        Entry::Occupied(existing) => {
            let current_node = existing.into_mut();

            match (&current_node.direct, &node.direct) {
                (Some(_), Some(_)) => {
                    log::warn!("duplicate direct dependency for {name}@{version}",);
                }

                (None, Some(_)) => {
                    current_node.direct = node.direct;
                }

                (_, _) => {}
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Lockfile {
    pub name: PackageName,
    pub version: Version,
    pub overrides: BTreeMap<OverrideKey, DependencySpecifiers>,

    pub graph: DependencyGraph,
}
