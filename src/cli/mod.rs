use crate::cli::auth::get_token;
use anyhow::Context;
use gix::bstr::BStr;
use pesde::{
    lockfile::{DownloadedGraph, Lockfile},
    names::{PackageName, PackageNames},
    source::{version_id::VersionId, workspace::specifier::VersionType},
    Project,
};
use serde::{ser::SerializeMap, Deserialize, Deserializer, Serializer};
use std::{
    collections::{BTreeMap, HashSet},
    fs::create_dir_all,
    str::FromStr,
};

pub mod auth;
pub mod commands;
pub mod config;
pub mod files;
pub mod scripts;
pub mod version;

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

pub fn up_to_date_lockfile(project: &Project) -> anyhow::Result<Option<Lockfile>> {
    let manifest = project.deser_manifest()?;
    let lockfile = match project.deser_lockfile() {
        Ok(lockfile) => lockfile,
        Err(pesde::errors::LockfileReadError::Io(e))
            if e.kind() == std::io::ErrorKind::NotFound =>
        {
            return Ok(None);
        }
        Err(e) => return Err(e.into()),
    };

    if manifest.overrides != lockfile.overrides {
        log::debug!("overrides are different");
        return Ok(None);
    }

    if manifest.target.kind() != lockfile.target {
        log::debug!("target kind is different");
        return Ok(None);
    }

    if manifest.name != lockfile.name || manifest.version != lockfile.version {
        log::debug!("name or version is different");
        return Ok(None);
    }

    let specs = lockfile
        .graph
        .iter()
        .flat_map(|(_, versions)| versions)
        .filter_map(|(_, node)| {
            node.node
                .direct
                .as_ref()
                .map(|(_, spec)| (spec, node.node.ty))
        })
        .collect::<HashSet<_>>();

    let same_dependencies = manifest
        .all_dependencies()
        .context("failed to get all dependencies")?
        .iter()
        .all(|(_, (spec, ty))| specs.contains(&(spec, *ty)));

    log::debug!("dependencies are the same: {same_dependencies}");

    Ok(if same_dependencies {
        Some(lockfile)
    } else {
        None
    })
}

#[derive(Debug, Clone)]
struct VersionedPackageName<V: FromStr = VersionId, N: FromStr = PackageNames>(N, Option<V>);

impl<V: FromStr<Err = E>, E: Into<anyhow::Error>, N: FromStr<Err = F>, F: Into<anyhow::Error>>
    FromStr for VersionedPackageName<V, N>
{
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, '@');
        let name = parts.next().unwrap();
        let version = parts
            .next()
            .map(FromStr::from_str)
            .transpose()
            .map_err(Into::into)?;

        Ok(VersionedPackageName(
            name.parse().map_err(Into::into)?,
            version,
        ))
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

#[derive(Debug, Clone)]
enum AnyPackageIdentifier<V: FromStr = VersionId, N: FromStr = PackageNames> {
    PackageName(VersionedPackageName<V, N>),
    Url((gix::Url, String)),
    Workspace(VersionedPackageName<VersionType, PackageName>),
}

impl<V: FromStr<Err = E>, E: Into<anyhow::Error>, N: FromStr<Err = F>, F: Into<anyhow::Error>>
    FromStr for AnyPackageIdentifier<V, N>
{
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(s) = s.strip_prefix("gh#") {
            let s = format!("https://github.com/{s}");
            let (repo, rev) = s.split_once('#').unwrap();

            Ok(AnyPackageIdentifier::Url((
                repo.try_into()?,
                rev.to_string(),
            )))
        } else if let Some(rest) = s.strip_prefix("workspace:") {
            Ok(AnyPackageIdentifier::Workspace(rest.parse()?))
        } else if s.contains(':') {
            let (url, rev) = s.split_once('#').unwrap();

            Ok(AnyPackageIdentifier::Url((
                url.try_into()?,
                rev.to_string(),
            )))
        } else {
            Ok(AnyPackageIdentifier::PackageName(s.parse()?))
        }
    }
}

pub fn parse_gix_url(s: &str) -> Result<gix::Url, gix::url::parse::Error> {
    s.try_into()
}

pub fn serialize_string_url_map<S: Serializer>(
    url: &BTreeMap<gix::Url, String>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(url.len()))?;
    for (k, v) in url {
        map.serialize_entry(&k.to_bstring().to_string(), v)?;
    }
    map.end()
}

pub fn deserialize_string_url_map<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<gix::Url, String>, D::Error> {
    BTreeMap::<String, String>::deserialize(deserializer)?
        .into_iter()
        .map(|(k, v)| {
            gix::Url::from_bytes(BStr::new(&k))
                .map(|k| (k, v))
                .map_err(serde::de::Error::custom)
        })
        .collect()
}
