use crate::{
    manifest::{DependencyType, OverrideKey, Target, TargetKind},
    names::{PackageName, PackageNames},
    source::{DependencySpecifiers, PackageRef, PackageRefs, VersionId},
};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::{btree_map::Entry, BTreeMap},
    path::{Path, PathBuf},
};

pub type Graph<Node> = BTreeMap<PackageNames, BTreeMap<VersionId, Node>>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DependencyGraphNode {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct: Option<(String, DependencySpecifiers)>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<PackageNames, (VersionId, String)>,
    pub ty: DependencyType,
    pub pkg_ref: PackageRefs,
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
    version: VersionId,
    mut node: DependencyGraphNode,
    is_top_level: bool,
) {
    if !is_top_level && node.direct.take().is_some() {
        log::debug!(
            "tried to insert {name}@{version} as direct dependency from a non top-level context",
        );
    }

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
    pub target: Target,
    #[serde(flatten)]
    pub node: DependencyGraphNode,
}

pub type DownloadedGraph = Graph<DownloadedDependencyGraphNode>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Lockfile {
    pub name: PackageName,
    pub version: Version,
    pub target: TargetKind,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<OverrideKey, DependencySpecifiers>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub graph: DownloadedGraph,
}
