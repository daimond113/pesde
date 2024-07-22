use crate::{
    linking::generator::get_file_types,
    lockfile::DownloadedGraph,
    manifest::{Manifest, ScriptName, Target},
    names::PackageNames,
    scripts::execute_script,
    source::PackageRef,
    Project, MANIFEST_FILE_NAME, PACKAGES_CONTAINER_NAME,
};
use semver::Version;
use std::{collections::BTreeMap, fs::create_dir_all};

pub mod generator;

fn read_manifest(path: &std::path::Path) -> Result<Manifest, errors::LinkingError> {
    let manifest = std::fs::read_to_string(path.join(MANIFEST_FILE_NAME))?;
    serde_yaml::from_str(&manifest)
        .map_err(|e| errors::LinkingError::DependencyManifest(path.display().to_string(), e))
}

impl Project {
    pub fn link_dependencies(&self, graph: &DownloadedGraph) -> Result<(), errors::LinkingError> {
        let manifest = self.deser_manifest()?;

        let mut package_types = BTreeMap::<&PackageNames, BTreeMap<&Version, Vec<String>>>::new();

        for (name, versions) in graph {
            for (version, node) in versions {
                let Some(lib_file) = node.target.lib_path() else {
                    continue;
                };

                let container_folder = node.node.container_folder(
                    &self
                        .path()
                        .join(node.node.base_folder(manifest.target.kind(), true))
                        .join(PACKAGES_CONTAINER_NAME),
                    name,
                    version,
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

                package_types
                    .entry(name)
                    .or_default()
                    .insert(version, types);

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
            for (version, node) in versions {
                let base_folder = self.path().join(
                    self.path()
                        .join(node.node.base_folder(manifest.target.kind(), true)),
                );
                create_dir_all(&base_folder)?;
                let base_folder = base_folder.canonicalize()?;
                let packages_container_folder = base_folder.join(PACKAGES_CONTAINER_NAME);

                let container_folder =
                    node.node
                        .container_folder(&packages_container_folder, name, version);

                let node_manifest = read_manifest(&container_folder)?;

                if let Some((alias, types)) = package_types
                    .get(name)
                    .and_then(|v| v.get(version))
                    .and_then(|types| node.node.direct.as_ref().map(|(alias, _)| (alias, types)))
                {
                    let module = generator::generate_linking_module(
                        &generator::get_require_path(
                            &node_manifest.target,
                            &base_folder,
                            &container_folder,
                            node.node.pkg_ref.use_new_structure(),
                        )?,
                        types,
                    );

                    std::fs::write(base_folder.join(format!("{alias}.luau")), module)?;
                }

                for (dependency_name, (dependency_version, dependency_alias)) in
                    &node.node.dependencies
                {
                    let Some(dependency_node) = graph
                        .get(dependency_name)
                        .and_then(|v| v.get(dependency_version))
                    else {
                        return Err(errors::LinkingError::DependencyNotFound(
                            dependency_name.to_string(),
                            dependency_version.to_string(),
                        ));
                    };

                    let dependency_container_folder = dependency_node.node.container_folder(
                        &packages_container_folder,
                        dependency_name,
                        dependency_version,
                    );

                    let dependency_manifest = read_manifest(&dependency_container_folder)?;

                    let linker_folder = container_folder
                        .join(dependency_node.node.base_folder(node.target.kind(), false));
                    create_dir_all(&linker_folder)?;
                    let linker_folder = linker_folder.canonicalize()?;

                    let linker_file = linker_folder.join(format!("{dependency_alias}.luau"));

                    let module = generator::generate_linking_module(
                        &generator::get_require_path(
                            &dependency_manifest.target,
                            &linker_file,
                            &dependency_container_folder,
                            node.node.pkg_ref.use_new_structure(),
                        )?,
                        package_types
                            .get(dependency_name)
                            .and_then(|v| v.get(dependency_version))
                            .unwrap(),
                    );

                    std::fs::write(linker_file, module)?;
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

        #[error("error deserializing manifest at {0}")]
        DependencyManifest(String, #[source] serde_yaml::Error),

        #[error("error interacting with filesystem")]
        Io(#[from] std::io::Error),

        #[error("dependency not found: {0}@{1}")]
        DependencyNotFound(String, String),

        #[error("library file at {0} not found")]
        LibFileNotFound(String),

        #[error("error parsing Luau script at {0}")]
        FullMoon(String, Vec<full_moon::Error>),

        #[error("error generating require path")]
        GetRequirePath(#[from] crate::linking::generator::errors::GetRequirePathError),

        #[cfg(feature = "roblox")]
        #[error("error generating roblox sync config for {0}")]
        GenerateRobloxSyncConfig(String, #[source] std::io::Error),
    }
}
