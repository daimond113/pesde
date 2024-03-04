use std::{
    fmt::Debug,
    fs::{read, File},
    path::{Path, PathBuf},
};
use thiserror::Error;

use crate::dependencies::DownloadError;
use crate::index::Index;
use crate::linking_file::LinkingDependenciesError;
use crate::{
    dependencies::resolution::ResolvedVersionsMap,
    manifest::{Manifest, ManifestReadError},
    LOCKFILE_FILE_NAME,
};

/// A pesde project
#[derive(Clone, Debug)]
pub struct Project<I: Index> {
    path: PathBuf,
    cache_path: PathBuf,
    index: I,
    manifest: Manifest,
    registry_auth_token: Option<String>,
}

/// Options for installing a project
pub struct InstallOptions {
    locked: bool,
    auto_download: bool,
    resolved_versions_map: Option<ResolvedVersionsMap>,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            locked: false,
            auto_download: true,
            resolved_versions_map: None,
        }
    }
}

impl InstallOptions {
    /// Creates a new set of install options (uses the Default implementation)
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes the installation to use the lockfile, and ensure that the lockfile is up-to-date
    pub fn locked(&self, locked: bool) -> Self {
        Self {
            locked,
            resolved_versions_map: self.resolved_versions_map.clone(),
            ..*self
        }
    }

    /// Makes the installation to automatically download the dependencies
    /// Having this set to false is only useful if you want to download the dependencies yourself. An example of this is the CLI's progress bar
    pub fn auto_download(&self, auto_download: bool) -> Self {
        Self {
            auto_download,
            resolved_versions_map: self.resolved_versions_map.clone(),
            ..*self
        }
    }

    /// Makes the installation to use the given resolved versions map
    /// Having this set to Some is only useful if you're using auto_download = false
    pub fn resolved_versions_map(&self, resolved_versions_map: ResolvedVersionsMap) -> Self {
        Self {
            resolved_versions_map: Some(resolved_versions_map),
            ..*self
        }
    }
}

/// An error that occurred while reading the lockfile
#[derive(Debug, Error)]
pub enum ReadLockfileError {
    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while deserializing the lockfile
    #[error("error deserializing lockfile")]
    LockfileDeser(#[source] serde_yaml::Error),
}

/// An error that occurred while downloading a project
#[derive(Debug, Error)]
pub enum InstallProjectError {
    /// An error that occurred while resolving the dependency tree
    #[error("failed to resolve dependency tree")]
    ResolveTree(#[from] crate::dependencies::resolution::ResolveError),

    /// An error that occurred while downloading a package
    #[error("failed to download package")]
    DownloadPackage(#[from] DownloadError),

    /// An error that occurred while applying patches
    #[error("error applying patches")]
    ApplyPatches(#[from] crate::patches::ApplyPatchesError),

    /// An error that occurred while linking dependencies
    #[error("failed to link dependencies")]
    Linking(#[from] LinkingDependenciesError),

    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while writing the lockfile
    #[error("failed to write lockfile")]
    LockfileSer(#[source] serde_yaml::Error),
}

impl<I: Index> Project<I> {
    /// Creates a new project
    pub fn new<P: AsRef<Path>, Q: AsRef<Path>>(
        path: P,
        cache_path: Q,
        index: I,
        manifest: Manifest,
        registry_auth_token: Option<String>,
    ) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            cache_path: cache_path.as_ref().to_path_buf(),
            index,
            manifest,
            registry_auth_token,
        }
    }

    /// Creates a new project from a path (manifest will be read from the path)
    pub fn from_path<P: AsRef<Path>, Q: AsRef<Path>>(
        path: P,
        cache_path: Q,
        index: I,
        registry_auth_token: Option<String>,
    ) -> Result<Self, ManifestReadError> {
        let manifest = Manifest::from_path(path.as_ref())?;

        Ok(Self::new(
            path,
            cache_path,
            index,
            manifest,
            registry_auth_token,
        ))
    }

    /// Returns the index of the project
    pub fn index(&self) -> &I {
        &self.index
    }

    /// Returns the manifest of the project
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Returns the cache directory of the project
    pub fn cache_dir(&self) -> &Path {
        &self.cache_path
    }

    /// Returns the path of the project
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the registry auth token of the project
    pub fn registry_auth_token(&self) -> Option<&String> {
        self.registry_auth_token.as_ref()
    }

    /// Returns the lockfile of the project
    pub fn lockfile(&self) -> Result<Option<ResolvedVersionsMap>, ReadLockfileError> {
        let lockfile_path = self.path.join(LOCKFILE_FILE_NAME);

        Ok(if lockfile_path.exists() {
            let lockfile_contents = read(&lockfile_path)?;
            let lockfile: ResolvedVersionsMap = serde_yaml::from_slice(&lockfile_contents)
                .map_err(ReadLockfileError::LockfileDeser)?;

            Some(lockfile)
        } else {
            None
        })
    }

    /// Downloads the project's dependencies, applies patches, and links the dependencies
    pub fn install(&self, install_options: InstallOptions) -> Result<(), InstallProjectError> {
        let map = match install_options.resolved_versions_map {
            Some(map) => map,
            None => self
                .manifest
                .dependency_tree(self, install_options.locked)?,
        };

        if install_options.auto_download {
            self.download(&map)?.wait()?;
        }

        self.apply_patches(&map)?;

        self.link_dependencies(&map)?;

        if !install_options.locked {
            serde_yaml::to_writer(File::create(self.path.join(LOCKFILE_FILE_NAME))?, &map)
                .map_err(InstallProjectError::LockfileSer)?;
        }

        Ok(())
    }
}
