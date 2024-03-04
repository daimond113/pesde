use log::debug;
use std::{fmt::Display, fs::create_dir_all, path::Path};

use semver::Version;
use serde::{de::IntoDeserializer, Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::{
    dependencies::{
        git::{GitDependencySpecifier, GitPackageRef},
        registry::{RegistryDependencySpecifier, RegistryPackageRef},
        resolution::ResolvedVersionsMap,
    },
    index::Index,
    manifest::Realm,
    multithread::MultithreadedJob,
    package_name::PackageName,
    project::{InstallProjectError, Project},
};

/// Git dependency related stuff
pub mod git;
/// Registry dependency related stuff
pub mod registry;
/// Resolution
pub mod resolution;

// To improve developer experience, we resolve the type of the dependency specifier with a custom deserializer, so that the user doesn't have to specify the type of the dependency
/// A dependency of a package
#[derive(Serialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum DependencySpecifier {
    /// A dependency that can be downloaded from a registry
    Registry(RegistryDependencySpecifier),
    /// A dependency that can be downloaded from a git repository
    Git(GitDependencySpecifier),
}

impl DependencySpecifier {
    /// Gets the name (or repository) of the specifier
    pub fn name(&self) -> String {
        match self {
            DependencySpecifier::Registry(registry) => registry.name.to_string(),
            DependencySpecifier::Git(git) => git.repo.to_string(),
        }
    }

    /// Gets the version (or revision) of the specifier
    pub fn version(&self) -> String {
        match self {
            DependencySpecifier::Registry(registry) => registry.version.to_string(),
            DependencySpecifier::Git(git) => git.rev.clone(),
        }
    }

    /// Gets the realm of the specifier
    pub fn realm(&self) -> Option<&Realm> {
        match self {
            DependencySpecifier::Registry(registry) => registry.realm.as_ref(),
            DependencySpecifier::Git(git) => git.realm.as_ref(),
        }
    }
}

impl<'de> Deserialize<'de> for DependencySpecifier {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let yaml = serde_yaml::Value::deserialize(deserializer)?;

        let result = if yaml.get("repo").is_some() {
            GitDependencySpecifier::deserialize(yaml.into_deserializer())
                .map(DependencySpecifier::Git)
        } else if yaml.get("name").is_some() {
            RegistryDependencySpecifier::deserialize(yaml.into_deserializer())
                .map(DependencySpecifier::Registry)
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
}

impl PackageRef {
    /// Gets the name of the package
    pub fn name(&self) -> &PackageName {
        match self {
            PackageRef::Registry(registry) => &registry.name,
            PackageRef::Git(git) => &git.name,
        }
    }

    /// Gets the version of the package
    pub fn version(&self) -> &Version {
        match self {
            PackageRef::Registry(registry) => &registry.version,
            PackageRef::Git(git) => &git.version,
        }
    }

    /// Downloads the package to the specified destination
    pub fn download<P: AsRef<Path>, I: Index>(
        &self,
        project: &Project<I>,
        dest: P,
    ) -> Result<(), DownloadError> {
        match self {
            PackageRef::Registry(registry) => registry
                .download(project, dest)
                .map_err(|e| DownloadError::Registry(e, Box::new(self.clone()))),
            PackageRef::Git(git) => git
                .download(project, dest)
                .map_err(|e| DownloadError::Git(e, Box::new(self.clone()))),
        }
    }
}

impl<I: Index> Project<I> {
    /// Downloads the project's dependencies
    pub fn download(
        &self,
        map: &ResolvedVersionsMap,
    ) -> Result<MultithreadedJob<DownloadError>, InstallProjectError> {
        let (job, tx) = MultithreadedJob::new();

        for (name, versions) in map.clone() {
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

                let project = self.clone();
                let tx = tx.clone();

                job.pool.execute(move || {
                    let result = resolved_package.pkg_ref.download(&project, source);
                    tx.send(result).unwrap();
                });
            }
        }

        Ok(job)
    }
}

impl Display for PackageRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.name(), self.version())
    }
}
