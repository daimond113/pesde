use crate::{
    lockfile::{insert_node, DependencyGraph, DependencyGraphNode},
    manifest::DependencyType,
    names::PackageNames,
    source::{
        pesde::PesdePackageSource, DependencySpecifiers, PackageRef, PackageSource, PackageSources,
    },
    Project, DEFAULT_INDEX_NAME,
};
use semver::Version;
use std::collections::{HashMap, HashSet, VecDeque};

impl Project {
    // TODO: implement dependency overrides
    pub fn dependency_graph(
        &self,
        previous_graph: Option<&DependencyGraph>,
    ) -> Result<DependencyGraph, Box<errors::DependencyGraphError>> {
        let manifest = self.deser_manifest().map_err(|e| Box::new(e.into()))?;

        let mut all_dependencies = manifest
            .all_dependencies()
            .map_err(|e| Box::new(e.into()))?;

        let mut all_specifiers = all_dependencies
            .clone()
            .into_iter()
            .map(|(alias, (spec, ty))| ((spec, ty), alias))
            .collect::<HashMap<_, _>>();

        let mut graph = DependencyGraph::default();

        if let Some(previous_graph) = previous_graph {
            for (name, versions) in previous_graph {
                for (version, node) in versions {
                    let Some((_, specifier)) = &node.direct else {
                        // this is not a direct dependency, will be added if it's still being used later
                        continue;
                    };

                    match all_specifiers.remove(&(specifier.clone(), node.ty)) {
                        Some(alias) => {
                            all_dependencies.remove(&alias);
                        }
                        None => {
                            // this dependency is no longer in the manifest, or it's type has changed
                            continue;
                        }
                    }

                    log::debug!("resolved {}@{} from old dependency graph", name, version);
                    insert_node(&mut graph, name.clone(), version.clone(), node.clone());

                    let mut queue = node
                        .dependencies
                        .iter()
                        .map(|(name, (version, _))| (name, version, 0usize))
                        .collect::<VecDeque<_>>();

                    while let Some((dep_name, dep_version, depth)) = queue.pop_front() {
                        if let Some(dep_node) = previous_graph
                            .get(dep_name)
                            .and_then(|v| v.get(dep_version))
                        {
                            log::debug!(
                                "{}resolved dependency {}@{} from {}@{}",
                                "\t".repeat(depth),
                                dep_name,
                                dep_version,
                                name,
                                version
                            );
                            insert_node(
                                &mut graph,
                                dep_name.clone(),
                                dep_version.clone(),
                                dep_node.clone(),
                            );

                            dep_node
                                .dependencies
                                .iter()
                                .map(|(name, (version, _))| (name, version, depth + 1))
                                .for_each(|dep| queue.push_back(dep));
                        } else {
                            log::warn!(
                                "dependency {}@{} from {}@{} not found in previous graph",
                                dep_name,
                                dep_version,
                                name,
                                version
                            );
                        }
                    }
                }
            }
        }

        let mut refreshed_sources = HashSet::new();
        let mut queue = all_dependencies
            .into_iter()
            .map(|(alias, (spec, ty))| (alias, spec, ty, None::<(PackageNames, Version)>, 0usize))
            .collect::<VecDeque<_>>();

        while let Some((alias, specifier, ty, dependant, depth)) = queue.pop_front() {
            log::debug!(
                "{}resolving {specifier} ({alias}) from {dependant:?}",
                "\t".repeat(depth)
            );
            let source = match &specifier {
                DependencySpecifiers::Pesde(specifier) => {
                    let index_url = if depth == 0 {
                        let index_name = specifier.index.as_deref().unwrap_or(DEFAULT_INDEX_NAME);
                        let index_url = manifest.indices.get(index_name).ok_or(
                            errors::DependencyGraphError::IndexNotFound(index_name.to_string()),
                        )?;

                        match index_url.as_str().try_into() {
                            Ok(url) => url,
                            Err(e) => {
                                return Err(Box::new(errors::DependencyGraphError::UrlParse(
                                    index_url.clone(),
                                    e,
                                )))
                            }
                        }
                    } else {
                        let index_url = specifier.index.clone().unwrap();

                        index_url
                            .clone()
                            .try_into()
                            .map_err(|e| errors::DependencyGraphError::InvalidIndex(index_url, e))?
                    };

                    PackageSources::Pesde(PesdePackageSource::new(index_url))
                }
            };

            if refreshed_sources.insert(source.clone()) {
                source.refresh(self).map_err(|e| Box::new(e.into()))?;
            }

            let (name, resolved) = source
                .resolve(&specifier, self)
                .map_err(|e| Box::new(e.into()))?;

            let Some(target_version) = graph
                .get(&name)
                .and_then(|versions| {
                    versions
                        .keys()
                        // only consider versions that are compatible with the specifier
                        .filter(|ver| resolved.contains_key(ver))
                        .max()
                })
                .or_else(|| resolved.last_key_value().map(|(ver, _)| ver))
                .cloned()
            else {
                log::warn!(
                    "{}could not find any version for {specifier} ({alias})",
                    "\t".repeat(depth)
                );
                continue;
            };

            let ty = if depth == 0 && ty == DependencyType::Peer {
                DependencyType::Standard
            } else {
                ty
            };

            if let Some((dependant_name, dependant_version)) = dependant {
                graph
                    .get_mut(&dependant_name)
                    .and_then(|versions| versions.get_mut(&dependant_version))
                    .and_then(|node| {
                        node.dependencies
                            .insert(name.clone(), (target_version.clone(), alias.clone()))
                    });
            }

            if let Some(already_resolved) = graph
                .get_mut(&name)
                .and_then(|versions| versions.get_mut(&target_version))
            {
                log::debug!(
                    "{}{}@{} already resolved",
                    "\t".repeat(depth),
                    name,
                    target_version
                );

                if already_resolved.ty == DependencyType::Peer && ty == DependencyType::Standard {
                    already_resolved.ty = ty;
                }

                continue;
            }

            let pkg_ref = &resolved[&target_version];
            let node = DependencyGraphNode {
                direct: if depth == 0 {
                    Some((alias.clone(), specifier.clone()))
                } else {
                    None
                },
                pkg_ref: pkg_ref.clone(),
                dependencies: Default::default(),
                ty,
            };
            insert_node(
                &mut graph,
                name.clone(),
                target_version.clone(),
                node.clone(),
            );

            log::debug!(
                "{}resolved {}@{} from new dependency graph",
                "\t".repeat(depth),
                name,
                target_version
            );

            for (dependency_alias, (dependency_spec, dependency_ty)) in
                pkg_ref.dependencies().clone()
            {
                if dependency_ty == DependencyType::Dev {
                    // dev dependencies of dependencies are not included in the graph
                    // they should not even be stored in the index, so this is just a check to avoid potential issues
                    continue;
                }

                queue.push_back((
                    dependency_alias,
                    dependency_spec,
                    dependency_ty,
                    Some((name.clone(), target_version.clone())),
                    depth + 1,
                ));
            }
        }

        for (name, versions) in &graph {
            for (version, node) in versions {
                if node.ty == DependencyType::Peer {
                    log::warn!("peer dependency {name}@{version} was not resolved");
                }
            }
        }

        Ok(graph)
    }
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum DependencyGraphError {
        #[error("failed to deserialize manifest")]
        ManifestRead(#[from] crate::errors::ManifestReadError),

        #[error("error getting all project dependencies")]
        AllDependencies(#[from] crate::manifest::errors::AllDependenciesError),

        #[error("index named {0} not found in manifest")]
        IndexNotFound(String),

        #[error("error parsing url {0} into git url")]
        UrlParse(url::Url, #[source] gix::url::parse::Error),

        #[error("index {0} cannot be parsed as a git url")]
        InvalidIndex(String, #[source] gix::url::parse::Error),

        #[error("error refreshing package source")]
        Refresh(#[from] crate::source::errors::RefreshError),

        #[error("error resolving package")]
        Resolve(#[from] crate::source::errors::ResolveError),
    }
}
