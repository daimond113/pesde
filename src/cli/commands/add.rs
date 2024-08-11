use std::str::FromStr;

use anyhow::Context;
use clap::Args;
use semver::VersionReq;

use pesde::{
    manifest::target::TargetKind,
    names::PackageNames,
    source::{
        pesde::{specifier::PesdeDependencySpecifier, PesdePackageSource},
        specifiers::DependencySpecifiers,
        traits::PackageSource,
        PackageSources,
    },
    Project, DEFAULT_INDEX_NAME,
};

use crate::cli::{config::read_config, VersionedPackageName};

#[derive(Debug, Args)]
pub struct AddCommand {
    /// The package name to add
    #[arg(index = 1)]
    name: VersionedPackageName<VersionReq>,

    /// The index in which to search for the package
    #[arg(short, long)]
    index: Option<String>,

    /// The target environment of the package
    #[arg(short, long)]
    target: Option<TargetKind>,

    /// The alias to use for the package
    #[arg(short, long)]
    alias: Option<String>,

    /// Whether to add the package as a peer dependency
    #[arg(short, long)]
    peer: bool,

    /// Whether to add the package as a dev dependency
    #[arg(short, long, conflicts_with = "peer")]
    dev: bool,
}

impl AddCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        let manifest = project
            .deser_manifest()
            .context("failed to read manifest")?;

        let source = match &self.name.0 {
            PackageNames::Pesde(_) => {
                let index = manifest
                    .indices
                    .get(self.index.as_deref().unwrap_or(DEFAULT_INDEX_NAME))
                    .cloned();

                if let Some(index) = self.index.as_ref().filter(|_| index.is_none()) {
                    log::error!("index {index} not found");
                    return Ok(());
                }

                let index = index.unwrap_or(read_config()?.default_index);

                PackageSources::Pesde(PesdePackageSource::new(index))
            }
            #[cfg(feature = "wally-compat")]
            PackageNames::Wally(_) => {
                let index = manifest
                    .wally_indices
                    .get(self.index.as_deref().unwrap_or(DEFAULT_INDEX_NAME))
                    .cloned();

                if let Some(index) = self.index.as_ref().filter(|_| index.is_none()) {
                    log::error!("wally index {index} not found");
                    return Ok(());
                }

                let index = index.unwrap_or(read_config()?.default_index);

                PackageSources::Wally(pesde::source::wally::WallyPackageSource::new(index))
            }
        };
        source
            .refresh(&project)
            .context("failed to refresh package source")?;

        let specifier = match &self.name.0 {
            PackageNames::Pesde(name) => DependencySpecifiers::Pesde(PesdeDependencySpecifier {
                name: name.clone(),
                version: self.name.1.unwrap_or(VersionReq::STAR),
                index: self.index,
                target: self.target,
            }),
            #[cfg(feature = "wally-compat")]
            PackageNames::Wally(name) => DependencySpecifiers::Wally(
                pesde::source::wally::specifier::WallyDependencySpecifier {
                    name: name.clone(),
                    version: self.name.1.unwrap_or(VersionReq::STAR),
                    index: self.index,
                },
            ),
        };

        let Some(version_id) = source
            .resolve(&specifier, &project, manifest.target.kind())
            .context("failed to resolve package")?
            .1
            .pop_last()
            .map(|(v_id, _)| v_id)
        else {
            log::error!(
                "no versions found for package: {} (current target: {}, try a different one)",
                self.name.0,
                manifest.target.kind()
            );

            return Ok(());
        };

        let project_target = manifest.target.kind();
        let mut manifest = toml_edit::DocumentMut::from_str(
            &project.read_manifest().context("failed to read manifest")?,
        )
        .context("failed to parse manifest")?;
        let dependency_key = if self.peer {
            "peer_dependencies"
        } else if self.dev {
            "dev_dependencies"
        } else {
            "dependencies"
        };

        let alias = self
            .alias
            .as_deref()
            .unwrap_or_else(|| self.name.0.as_str().1);

        match specifier {
            DependencySpecifiers::Pesde(spec) => {
                manifest[dependency_key][alias]["name"] =
                    toml_edit::value(spec.name.clone().to_string());
                manifest[dependency_key][alias]["version"] =
                    toml_edit::value(format!("^{}", version_id.version()));

                if *version_id.target() != project_target {
                    manifest[dependency_key][alias]["target"] =
                        toml_edit::value(version_id.target().to_string());
                }

                if let Some(index) = spec.index.filter(|i| i != DEFAULT_INDEX_NAME) {
                    manifest[dependency_key][alias]["index"] = toml_edit::value(index);
                }

                println!(
                    "added {}@{} {} to {}",
                    spec.name,
                    version_id.version(),
                    version_id.target(),
                    dependency_key
                );
            }
            #[cfg(feature = "wally-compat")]
            DependencySpecifiers::Wally(spec) => {
                manifest[dependency_key][alias]["wally"] =
                    toml_edit::value(spec.name.clone().to_string());
                manifest[dependency_key][alias]["version"] =
                    toml_edit::value(format!("^{}", version_id.version()));

                if let Some(index) = spec.index.filter(|i| i != DEFAULT_INDEX_NAME) {
                    manifest[dependency_key][alias]["index"] = toml_edit::value(index);
                }

                println!(
                    "added wally {}@{} to {}",
                    spec.name,
                    version_id.version(),
                    dependency_key
                );
            }
            DependencySpecifiers::Git(_) => {
                unreachable!("git dependencies are not supported in the add command");
            }
        }

        project
            .write_manifest(manifest.to_string())
            .context("failed to write manifest")?;

        Ok(())
    }
}
