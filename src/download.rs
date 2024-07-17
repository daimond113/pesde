use std::{
    collections::{BTreeMap, HashSet},
    fs::create_dir_all,
};

use crate::{
    lockfile::{DependencyGraph, DownloadedDependencyGraphNode, DownloadedGraph},
    source::{pesde::PesdePackageSource, PackageRefs, PackageSource, PackageSources},
    Project, PACKAGES_CONTAINER_NAME,
};

impl Project {
    pub fn download_graph(
        &self,
        graph: &DependencyGraph,
        refreshed_sources: &mut HashSet<PackageSources>,
    ) -> Result<DownloadedGraph, errors::DownloadGraphError> {
        let manifest = self.deser_manifest()?;

        let mut downloaded_graph: DownloadedGraph = BTreeMap::new();

        for (name, versions) in graph {
            for (version, node) in versions {
                let source = match &node.pkg_ref {
                    PackageRefs::Pesde(pkg_ref) => {
                        PackageSources::Pesde(PesdePackageSource::new(pkg_ref.index_url.clone()))
                    }
                };

                if refreshed_sources.insert(source.clone()) {
                    source.refresh(self).map_err(Box::new)?;
                }

                let container_folder = node.container_folder(
                    &self
                        .path()
                        .join(node.base_folder(manifest.target.kind(), true))
                        .join(PACKAGES_CONTAINER_NAME),
                    name,
                    version,
                );

                create_dir_all(&container_folder)?;

                let target = source.download(&node.pkg_ref, &container_folder, self)?;

                downloaded_graph.entry(name.clone()).or_default().insert(
                    version.clone(),
                    DownloadedDependencyGraphNode {
                        node: node.clone(),
                        target,
                    },
                );
            }
        }

        Ok(downloaded_graph)
    }
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum DownloadGraphError {
        #[error("error deserializing project manifest")]
        ManifestDeserializationFailed(#[from] crate::errors::ManifestReadError),

        #[error("failed to refresh package source")]
        RefreshFailed(#[from] Box<crate::source::errors::RefreshError>),

        #[error("error interacting with filesystem")]
        Io(#[from] std::io::Error),

        #[error("failed to download package")]
        DownloadFailed(#[from] crate::source::errors::DownloadError),
    }
}
