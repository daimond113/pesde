pub mod auth;
pub mod commands;
pub mod config;
pub mod files;
pub mod scripts;
pub mod version;

use crate::cli::auth::get_token;
use anyhow::Context;
use pesde::{
    lockfile::DownloadedGraph, names::PackageNames, source::version_id::VersionId, Project,
};
use std::{collections::HashSet, fs::create_dir_all, str::FromStr};

pub const HOME_DIR: &str = concat!(".", env!("CARGO_PKG_NAME"));

pub fn home_dir() -> anyhow::Result<std::path::PathBuf> {
    Ok(dirs::home_dir()
        .context("failed to get home directory")?
        .join(HOME_DIR))
}

pub fn bin_dir() -> anyhow::Result<std::path::PathBuf> {
    let bin_dir = home_dir()?.join("bin");
    create_dir_all(&bin_dir).context("failed to create bin folder")?;
    Ok(bin_dir)
}

pub trait IsUpToDate {
    fn is_up_to_date(&self, strict: bool) -> anyhow::Result<bool>;
}

impl IsUpToDate for Project {
    fn is_up_to_date(&self, strict: bool) -> anyhow::Result<bool> {
        let manifest = self.deser_manifest()?;
        let lockfile = match self.deser_lockfile() {
            Ok(lockfile) => lockfile,
            Err(pesde::errors::LockfileReadError::Io(e))
                if e.kind() == std::io::ErrorKind::NotFound =>
            {
                return Ok(false);
            }
            Err(e) => return Err(e.into()),
        };

        if manifest.overrides != lockfile.overrides {
            log::debug!("overrides are different");
            return Ok(false);
        }

        if manifest.target.kind() != lockfile.target {
            log::debug!("target kind is different");
            return Ok(false);
        }

        if !strict {
            return Ok(true);
        }

        if manifest.name != lockfile.name || manifest.version != lockfile.version {
            log::debug!("name or version is different");
            return Ok(false);
        }

        let specs = lockfile
            .graph
            .into_iter()
            .flat_map(|(_, versions)| versions)
            .filter_map(|(_, node)| match node.node.direct {
                Some((_, spec)) => Some((spec, node.node.ty)),
                None => None,
            })
            .collect::<HashSet<_>>();

        let same_dependencies = manifest
            .all_dependencies()
            .context("failed to get all dependencies")?
            .iter()
            .all(|(_, (spec, ty))| specs.contains(&(spec.clone(), *ty)));

        log::debug!("dependencies are the same: {same_dependencies}");

        Ok(same_dependencies)
    }
}

#[derive(Debug, Clone)]
struct VersionedPackageName(PackageNames, Option<VersionId>);

impl FromStr for VersionedPackageName {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, '@');
        let name = parts.next().unwrap();
        let version = parts.next().map(VersionId::from_str).transpose()?;

        Ok(VersionedPackageName(name.parse()?, version))
    }
}

impl VersionedPackageName {
    fn get(self, graph: &DownloadedGraph) -> anyhow::Result<(PackageNames, VersionId)> {
        let version_id = match self.1 {
            Some(version) => version,
            None => {
                let versions = graph.get(&self.0).context("package not found in graph")?;
                if versions.len() == 1 {
                    let version = versions.keys().next().unwrap().clone();
                    log::debug!("only one version found, using {version}");
                    version
                } else {
                    anyhow::bail!(
                        "multiple versions found, please specify one of: {}",
                        versions
                            .keys()
                            .map(|v| v.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
        };

        Ok((self.0, version_id))
    }
}

pub fn parse_gix_url(s: &str) -> Result<gix::Url, gix::url::parse::Error> {
    s.try_into()
}
