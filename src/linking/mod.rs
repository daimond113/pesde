use crate::{
    linking::generator::get_file_types,
    lockfile::DownloadedGraph,
    manifest::target::Target,
    names::PackageNames,
    scripts::{execute_script, ScriptName},
    source::{fs::store_in_cas, traits::PackageRef, version_id::VersionId},
    Project, PACKAGES_CONTAINER_NAME,
};
use std::{
    collections::BTreeMap,
    fs::create_dir_all,
    path::{Path, PathBuf},
};

pub mod generator;

fn create_and_canonicalize<P: AsRef<Path>>(path: P) -> std::io::Result<PathBuf> {
    let p = path.as_ref();
    create_dir_all(p)?;
    p.canonicalize()
}

fn write_cas(destination: PathBuf, cas_dir: &Path, contents: &str) -> std::io::Result<()> {
    let cas_path = store_in_cas(cas_dir, contents)?.1;

    std::fs::hard_link(cas_path, destination)
}

impl Project {
    pub fn link_dependencies(&self, graph: &DownloadedGraph) -> Result<(), errors::LinkingError> {
        let manifest = self.deser_manifest()?;

        let mut package_types = BTreeMap::<&PackageNames, BTreeMap<&VersionId, Vec<String>>>::new();

        for (name, versions) in graph {
            for (version_id, node) in versions {
                let Some(lib_file) = node.target.lib_path() else {
                    continue;
                };

                let container_folder = node.node.container_folder(
                    &self
                        .path()
                        .join(node.node.base_folder(manifest.target.kind(), true))
                        .join(PACKAGES_CONTAINER_NAME),
                    name,
                    version_id.version(),
                );

                let lib_file = lib_file.to_path(&container_folder);

                let contents = match std::fs::read_to_string(&lib_file) {
                    Ok(contents) => contents,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return Err(errors::LinkingError::LibFileNotFound(
                            lib_file.display().to_string(),
                        ));
                    }
                    Err(e) => return Err(e.into()),
                };

                let types = match get_file_types(&contents) {
                    Ok(types) => types,
                    Err(e) => {
                        return Err(errors::LinkingError::FullMoon(
                            lib_file.display().to_string(),
                            e,
                        ))
                    }
                };

                log::debug!("{name}@{version_id} has {} exported types", types.len());

                package_types
                    .entry(name)
                    .or_default()
                    .insert(version_id, types);

                #[cfg(feature = "roblox")]
                if let Target::Roblox { build_files, .. } = &node.target {
                    let script_name = ScriptName::RobloxSyncConfigGenerator.to_string();

                    let Some(script_path) = manifest.scripts.get(&script_name) else {
                        log::warn!("not having a `{script_name}` script in the manifest might cause issues with Roblox linking");
                        continue;
                    };

                    execute_script(
                        Some(&script_name),
                        &script_path.to_path(self.path()),
                        build_files,
                        &container_folder,
                        false,
                    )
                    .map_err(|e| {
                        errors::LinkingError::GenerateRobloxSyncConfig(
                            container_folder.display().to_string(),
                            e,
                        )
                    })?;
                }
            }
        }

        for (name, versions) in graph {
            for (version_id, node) in versions {
                let base_folder = create_and_canonicalize(
                    self.path().join(
                        self.path()
                            .join(node.node.base_folder(manifest.target.kind(), true)),
                    ),
                )?;
                let packages_container_folder = base_folder.join(PACKAGES_CONTAINER_NAME);

                let container_folder = node.node.container_folder(
                    &packages_container_folder,
                    name,
                    version_id.version(),
                );

                if let Some((alias, types)) = package_types
                    .get(name)
                    .and_then(|v| v.get(version_id))
                    .and_then(|types| node.node.direct.as_ref().map(|(alias, _)| (alias, types)))
                {
                    if let Some(lib_file) = node.target.lib_path() {
                        write_cas(
                            base_folder.join(format!("{alias}.luau")),
                            self.cas_dir(),
                            &generator::generate_lib_linking_module(
                                &generator::get_lib_require_path(
                                    &node.target.kind(),
                                    &base_folder,
                                    lib_file,
                                    &container_folder,
                                    node.node.pkg_ref.use_new_structure(),
                                ),
                                types,
                            ),
                        )?;
                    };

                    if let Some(bin_file) = node.target.bin_path() {
                        write_cas(
                            base_folder.join(format!("{alias}.bin.luau")),
                            self.cas_dir(),
                            &generator::generate_bin_linking_module(
                                &generator::get_bin_require_path(
                                    &base_folder,
                                    bin_file,
                                    &container_folder,
                                ),
                            ),
                        )?;
                    }
                }

                for (dependency_name, (dependency_version_id, dependency_alias)) in
                    &node.node.dependencies
                {
                    let Some(dependency_node) = graph
                        .get(dependency_name)
                        .and_then(|v| v.get(dependency_version_id))
                    else {
                        return Err(errors::LinkingError::DependencyNotFound(
                            dependency_name.to_string(),
                            dependency_version_id.to_string(),
                        ));
                    };

                    let Some(lib_file) = dependency_node.target.lib_path() else {
                        continue;
                    };

                    let linker_folder = create_and_canonicalize(
                        container_folder
                            .join(dependency_node.node.base_folder(node.target.kind(), false)),
                    )?;

                    write_cas(
                        linker_folder.join(format!("{dependency_alias}.luau")),
                        self.cas_dir(),
                        &generator::generate_lib_linking_module(
                            &generator::get_lib_require_path(
                                &dependency_node.target.kind(),
                                &linker_folder,
                                lib_file,
                                &dependency_node.node.container_folder(
                                    &packages_container_folder,
                                    dependency_name,
                                    dependency_version_id.version(),
                                ),
                                node.node.pkg_ref.use_new_structure(),
                            ),
                            package_types
                                .get(dependency_name)
                                .and_then(|v| v.get(dependency_version_id))
                                .unwrap(),
                        ),
                    )?;
                }
            }
        }

        Ok(())
    }
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum LinkingError {
        #[error("error deserializing project manifest")]
        Manifest(#[from] crate::errors::ManifestReadError),

        #[error("error interacting with filesystem")]
        Io(#[from] std::io::Error),

        #[error("dependency not found: {0}@{1}")]
        DependencyNotFound(String, String),

        #[error("library file at {0} not found")]
        LibFileNotFound(String),

        #[error("error parsing Luau script at {0}")]
        FullMoon(String, Vec<full_moon::Error>),

        #[cfg(feature = "roblox")]
        #[error("error generating roblox sync config for {0}")]
        GenerateRobloxSyncConfig(String, #[source] std::io::Error),
    }
}