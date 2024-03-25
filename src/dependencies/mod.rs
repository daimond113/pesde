use std::{
    fmt::Display,
    fs::create_dir_all,
    path::{Path, PathBuf},
    sync::Arc,
};

use cfg_if::cfg_if;
use log::debug;
use reqwest::header::AUTHORIZATION;
use semver::Version;
use serde::{de::IntoDeserializer, Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use thiserror::Error;
use url::Url;

use crate::{
    dependencies::{
        git::{GitDependencySpecifier, GitPackageRef},
        registry::{RegistryDependencySpecifier, RegistryPackageRef},
        resolution::RootLockfileNode,
    },
    index::{CredentialsFn, Index},
    manifest::{Manifest, Realm},
    multithread::MultithreadedJob,
    package_name::PackageName,
    project::{get_index, get_index_by_url, InstallProjectError, Project},
};

/// Git dependency related stuff
pub mod git;
/// Registry dependency related stuff
pub mod registry;
/// Resolution
pub mod resolution;
/// Wally dependency related stuff
#[cfg(feature = "wally")]
pub mod wally;

// To improve developer experience, we resolve the type of the dependency specifier with a custom deserializer, so that the user doesn't have to specify the type of the dependency
/// A dependency of a package
#[derive(Serialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum DependencySpecifier {
    /// A dependency that can be downloaded from a registry
    Registry(RegistryDependencySpecifier),
    /// A dependency that can be downloaded from a git repository
    Git(GitDependencySpecifier),
    /// A dependency that can be downloaded from a wally registry
    #[cfg(feature = "wally")]
    Wally(wally::WallyDependencySpecifier),
}

impl DependencySpecifier {
    /// Gets the name (or repository) of the specifier
    pub fn name(&self) -> String {
        match self {
            DependencySpecifier::Registry(registry) => registry.name.to_string(),
            DependencySpecifier::Git(git) => git.repo.to_string(),
            #[cfg(feature = "wally")]
            DependencySpecifier::Wally(wally) => wally.name.to_string(),
        }
    }

    /// Gets the version (or revision) of the specifier
    pub fn version(&self) -> String {
        match self {
            DependencySpecifier::Registry(registry) => registry.version.to_string(),
            DependencySpecifier::Git(git) => git.rev.clone(),
            #[cfg(feature = "wally")]
            DependencySpecifier::Wally(wally) => wally.version.to_string(),
        }
    }

    /// Gets the realm of the specifier
    pub fn realm(&self) -> Option<&Realm> {
        match self {
            DependencySpecifier::Registry(registry) => registry.realm.as_ref(),
            DependencySpecifier::Git(git) => git.realm.as_ref(),
            #[cfg(feature = "wally")]
            DependencySpecifier::Wally(wally) => wally.realm.as_ref(),
        }
    }
}

impl<'de> Deserialize<'de> for DependencySpecifier {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let yaml = Value::deserialize(deserializer)?;

        let result = if yaml.get("repo").is_some() {
            GitDependencySpecifier::deserialize(yaml.into_deserializer())
                .map(DependencySpecifier::Git)
        } else if yaml.get("name").is_some() {
            RegistryDependencySpecifier::deserialize(yaml.into_deserializer())
                .map(DependencySpecifier::Registry)
        } else if yaml.get("wally").is_some() {
            cfg_if! {
                if #[cfg(feature = "wally")] {
                    wally::WallyDependencySpecifier::deserialize(yaml.into_deserializer())
                        .map(DependencySpecifier::Wally)
                } else {
                    Err(serde::de::Error::custom("wally is not enabled"))
                }
            }
        } else {
            Err(serde::de::Error::custom("invalid dependency"))
        };

        result.map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}

// Here we don't use a custom deserializer, because this is exposed to the user only from the lock file, which mustn't be edited manually anyway
/// A reference to a package
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum PackageRef {
    /// A reference to a package that can be downloaded from a registry
    Registry(RegistryPackageRef),
    /// A reference to a package that can be downloaded from a git repository
    Git(GitPackageRef),
    /// A reference to a package that can be downloaded from a wally registry
    #[cfg(feature = "wally")]
    Wally(wally::WallyPackageRef),
}

/// An error that occurred while downloading a package
#[derive(Debug, Error)]
pub enum DownloadError {
    /// An error that occurred while downloading a package from a registry
    #[error("error downloading package {1} from registry")]
    Registry(#[source] registry::RegistryDownloadError, Box<PackageRef>),

    /// An error that occurred while downloading a package from a git repository
    #[error("error downloading package {1} from git repository")]
    Git(#[source] git::GitDownloadError, Box<PackageRef>),

    /// An error that occurred while downloading a package from a wally registry
    #[cfg(feature = "wally")]
    #[error("error downloading package {1} from wally registry")]
    Wally(#[source] wally::WallyDownloadError, Box<PackageRef>),

    /// A URL is required for this type of package reference
    #[error("a URL is required for this type of package reference")]
    UrlRequired,
}

/// An error that occurred while resolving a URL
#[derive(Debug, Error)]
pub enum UrlResolveError {
    /// An error that occurred while resolving a URL of a registry package
    #[error("error resolving URL of registry package")]
    Registry(#[from] registry::RegistryUrlResolveError),

    /// An error that occurred while resolving a URL of a wally package
    #[cfg(feature = "wally")]
    #[error("error resolving URL of wally package")]
    Wally(#[from] wally::ResolveWallyUrlError),
}

impl PackageRef {
    /// Gets the name of the package
    pub fn name(&self) -> PackageName {
        match self {
            PackageRef::Registry(registry) => PackageName::Standard(registry.name.clone()),
            PackageRef::Git(git) => PackageName::Standard(git.name.clone()),
            #[cfg(feature = "wally")]
            PackageRef::Wally(wally) => PackageName::Wally(wally.name.clone()),
        }
    }

    /// Gets the version of the package
    pub fn version(&self) -> &Version {
        match self {
            PackageRef::Registry(registry) => &registry.version,
            PackageRef::Git(git) => &git.version,
            #[cfg(feature = "wally")]
            PackageRef::Wally(wally) => &wally.version,
        }
    }

    /// Returns the URL of the index
    pub fn index_url(&self) -> Option<Url> {
        match self {
            PackageRef::Registry(registry) => Some(registry.index_url.clone()),
            PackageRef::Git(_) => None,
            #[cfg(feature = "wally")]
            PackageRef::Wally(wally) => Some(wally.index_url.clone()),
        }
    }

    /// Resolves the URL of the package
    pub fn resolve_url(&self, project: &mut Project) -> Result<Option<Url>, UrlResolveError> {
        Ok(match &self {
            PackageRef::Registry(registry) => Some(registry.resolve_url(project.indices())?),
            PackageRef::Git(_) => None,
            #[cfg(feature = "wally")]
            PackageRef::Wally(wally) => {
                let cache_dir = project.cache_dir().to_path_buf();
                Some(wally.resolve_url(&cache_dir, project.indices_mut())?)
            }
        })
    }

    /// Gets the index of the package
    pub fn get_index<'a>(&self, project: &'a Project) -> &'a dyn Index {
        match &self.index_url() {
            Some(url) => get_index_by_url(project.indices(), url),
            None => get_index(project.indices(), None),
        }
    }

    /// Downloads the package to the specified destination
    pub fn download<P: AsRef<Path>>(
        &self,
        reqwest_client: &reqwest::blocking::Client,
        registry_auth_token: Option<String>,
        url: Option<&Url>,
        credentials_fn: Option<Arc<CredentialsFn>>,
        dest: P,
    ) -> Result<(), DownloadError> {
        match self {
            PackageRef::Registry(registry) => registry
                .download(
                    reqwest_client,
                    url.ok_or(DownloadError::UrlRequired)?,
                    registry_auth_token,
                    dest,
                )
                .map_err(|e| DownloadError::Registry(e, Box::new(self.clone()))),
            PackageRef::Git(git) => git
                .download(dest, credentials_fn)
                .map_err(|e| DownloadError::Git(e, Box::new(self.clone()))),
            #[cfg(feature = "wally")]
            PackageRef::Wally(wally) => wally
                .download(
                    reqwest_client,
                    url.ok_or(DownloadError::UrlRequired)?,
                    registry_auth_token,
                    dest,
                )
                .map_err(|e| DownloadError::Wally(e, Box::new(self.clone()))),
        }
    }
}

/// An error that occurred while converting a manifest
#[derive(Debug, Error)]
pub enum ConvertManifestsError {
    /// An error that occurred while converting the manifest
    #[error("error converting the manifest")]
    Manifest(#[from] crate::manifest::ManifestConvertError),

    /// An error that occurred while reading the sourcemap
    #[error("error reading the sourcemap")]
    Sourcemap(#[from] std::io::Error),

    /// An error that occurred while parsing the sourcemap
    #[cfg(feature = "wally")]
    #[error("error parsing the sourcemap")]
    Parse(#[from] serde_json::Error),

    /// An error that occurred while writing the manifest
    #[error("error writing the manifest")]
    Write(#[from] serde_yaml::Error),

    /// A manifest is not present in a dependency, and the wally feature is not enabled
    #[cfg(not(feature = "wally"))]
    #[error("wally feature is not enabled, but the manifest is not present in the dependency")]
    ManifestNotPresent,
}

impl Project {
    /// Downloads the project's dependencies
    pub fn download(
        &mut self,
        lockfile: &RootLockfileNode,
    ) -> Result<MultithreadedJob<DownloadError>, InstallProjectError> {
        let (job, tx) = MultithreadedJob::new();

        for (name, versions) in lockfile.children.clone() {
            for (version, resolved_package) in versions {
                let (_, source) = resolved_package.directory(self.path());

                if source.exists() {
                    debug!("package {name}@{version} already downloaded, skipping...");
                    continue;
                }

                debug!(
                    "downloading package {name}@{version} to {}",
                    source.display()
                );

                create_dir_all(&source)?;

                let reqwest_client = self.reqwest_client.clone();
                let url = resolved_package.pkg_ref.resolve_url(self)?;
                let index = resolved_package.pkg_ref.get_index(self);
                let registry_auth_token = index.registry_auth_token().map(|t| t.to_string());
                let credentials_fn = index.credentials_fn().cloned();

                job.execute(&tx, move || {
                    resolved_package.pkg_ref.download(
                        &reqwest_client,
                        registry_auth_token,
                        url.as_ref(),
                        credentials_fn,
                        source,
                    )
                });
            }
        }

        Ok(job)
    }

    /// Converts the manifests of the project's dependencies
    #[cfg(feature = "wally")]
    pub fn convert_manifests<F: Fn(PathBuf)>(
        &self,
        lockfile: &RootLockfileNode,
        generate_sourcemap: F,
    ) -> Result<(), ConvertManifestsError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SourcemapNode {
            #[serde(default)]
            file_paths: Vec<relative_path::RelativePathBuf>,
        }

        for versions in lockfile.children.values() {
            for resolved_package in versions.values() {
                let source = match &resolved_package.pkg_ref {
                    PackageRef::Wally(_) | PackageRef::Git(_) => {
                        resolved_package.directory(self.path()).1
                    }
                    _ => continue,
                };

                let mut manifest = Manifest::from_path_or_convert(&source)?;

                generate_sourcemap(source.to_path_buf());

                let sourcemap = source.join("sourcemap.json");
                let sourcemap: SourcemapNode = if sourcemap.exists() {
                    serde_json::from_str(&std::fs::read_to_string(&sourcemap)?)?
                } else {
                    log::warn!("sourcemap for {resolved_package} not found, skipping...");
                    continue;
                };

                manifest.exports.lib = sourcemap
                    .file_paths
                    .into_iter()
                    .find(|path| {
                        path.extension()
                            .is_some_and(|ext| ext == "lua" || ext == "luau")
                    })
                    .or_else(|| Some(relative_path::RelativePathBuf::from("true")));

                serde_yaml::to_writer(
                    &std::fs::File::create(&source.join(crate::MANIFEST_FILE_NAME))?,
                    &manifest,
                )?;
            }
        }

        Ok(())
    }

    /// Errors if dependencies don't have manifests, enable the `wally` feature to convert them
    #[cfg(not(feature = "wally"))]
    pub fn convert_manifests<F: Fn(PathBuf)>(
        &self,
        lockfile: &RootLockfileNode,
        _generate_sourcemap: F,
    ) -> Result<(), ConvertManifestsError> {
        for versions in lockfile.children.values() {
            for resolved_package in versions.values() {
                let source = match &resolved_package.pkg_ref {
                    PackageRef::Git(_) => resolved_package.directory(self.path()).1,
                    _ => continue,
                };

                if Manifest::from_path_or_convert(&source).is_err() {
                    return Err(ConvertManifestsError::ManifestNotPresent);
                }
            }
        }

        Ok(())
    }
}

impl Display for PackageRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.name(), self.version())
    }
}

pub(crate) fn maybe_authenticated_request(
    reqwest_client: &reqwest::blocking::Client,
    url: &str,
    registry_auth_token: Option<String>,
) -> reqwest::blocking::RequestBuilder {
    let mut builder = reqwest_client.get(url);
    debug!("sending request to {}", url);

    if let Some(token) = registry_auth_token {
        let hidden_token = token
            .chars()
            .enumerate()
            .map(|(i, c)| if i <= 8 { c } else { '*' })
            .collect::<String>();
        debug!("with registry token {hidden_token}");
        builder = builder.header(AUTHORIZATION, format!("Bearer {token}"));
    }

    builder
}
