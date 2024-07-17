use crate::{
    manifest::{DependencyType, OverrideKey, Target, TargetKind},
    names::{PackageName, PackageNames},
    source::{DependencySpecifiers, PackageRef, PackageRefs},
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::{btree_map::Entry, BTreeMap},
    path::{Path, PathBuf},
};

pub type Graph<Node> = BTreeMap<PackageNames, BTreeMap<Version, Node>>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependencyGraphNode {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct: Option<(String, DependencySpecifiers)>,
    pub pkg_ref: PackageRefs,
    pub dependencies: BTreeMap<PackageNames, (Version, String)>,
    pub ty: DependencyType,
}

impl DependencyGraphNode {
    pub fn base_folder(&self, project_target: TargetKind, is_top_level: bool) -> String {
        if is_top_level || self.pkg_ref.use_new_structure() {
            project_target.packages_folder(&self.pkg_ref.target_kind())
        } else {
            "..".to_string()
        }
    }

    pub fn container_folder<P: AsRef<Path>>(
        &self,
        path: &P,
        name: &PackageNames,
        version: &Version,
    ) -> PathBuf {
        path.as_ref()
            .join(name.escaped())
            .join(version.to_string())
            .join(name.as_str().1)
    }
}

pub type DependencyGraph = Graph<DependencyGraphNode>;

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
pub struct DownloadedDependencyGraphNode {
    pub node: DependencyGraphNode,
    pub target: Target,
}

pub type DownloadedGraph = Graph<DownloadedDependencyGraphNode>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Lockfile {
    pub name: PackageName,
    pub version: Version,
    pub overrides: BTreeMap<OverrideKey, DependencySpecifiers>,

    pub graph: DependencyGraph,
}
