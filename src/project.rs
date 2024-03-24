use log::{error, warn};
use std::{
    collections::HashMap,
    fmt::Debug,
    fs::{read, File},
    path::{Path, PathBuf},
};

use thiserror::Error;
use url::Url;

use crate::{
    dependencies::{resolution::ResolvedVersionsMap, DownloadError, UrlResolveError},
    index::Index,
    linking_file::LinkingDependenciesError,
    manifest::{Manifest, ManifestReadError},
    LOCKFILE_FILE_NAME,
};

/// A map of indices
pub type Indices = HashMap<String, Box<dyn Index>>;

/// A pesde project
#[derive(Debug)]
pub struct Project {
    path: PathBuf,
    cache_path: PathBuf,
    indices: Indices,
    manifest: Manifest,
    pub(crate) reqwest_client: reqwest::blocking::Client,
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

    /// An error that occurred while resolving the url of a package
    #[error("failed to resolve package URL")]
    UrlResolve(#[from] UrlResolveError),
}

/// The name of the default index to use
pub const DEFAULT_INDEX_NAME: &str = "default";

pub(crate) fn get_index<'a>(indices: &'a Indices, index_name: Option<&str>) -> &'a dyn Index {
    indices
        .get(index_name.unwrap_or(DEFAULT_INDEX_NAME))
        .or_else(|| {
            warn!(
                "index `{}` not found, using default index",
                index_name.unwrap_or("<not provided>")
            );
            indices.get(DEFAULT_INDEX_NAME)
        })
        .unwrap()
        .as_ref()
}

pub(crate) fn get_index_by_url<'a>(indices: &'a Indices, url: &Url) -> &'a dyn Index {
    indices
        .values()
        .find(|index| index.url() == url)
        .map(|index| index.as_ref())
        .unwrap_or_else(|| get_index(indices, None))
}

#[cfg(feature = "wally")]
pub(crate) fn get_wally_index<'a>(
    indices: &'a mut Indices,
    url: &Url,
    path: Option<&Path>,
) -> Result<&'a crate::index::WallyIndex, crate::index::RefreshError> {
    if !indices.contains_key(url.as_str()) {
        let default_index = indices.get(DEFAULT_INDEX_NAME).unwrap();
        let default_token = default_index.registry_auth_token().map(|t| t.to_string());
        let default_credentials_fn = default_index.credentials_fn().cloned();

        let index = crate::index::WallyIndex::new(
            url.clone(),
            default_token,
            path.expect("index should already exist by now"),
            default_credentials_fn,
        );

        match index.refresh() {
            Ok(_) => {
                indices.insert(url.as_str().to_string(), Box::new(index));
            }
            Err(e) => {
                error!("failed to refresh wally index: {e}");
                return Err(e);
            }
        }
    }

    Ok(indices
        .get(url.as_str())
        .unwrap()
        .as_any()
        .downcast_ref()
        .unwrap())
}

/// An error that occurred while creating a new project
#[derive(Debug, Error)]
pub enum NewProjectError {
    /// A default index was not provided
    #[error("default index not provided")]
    DefaultIndexNotProvided,
}

/// An error that occurred while creating a project from a path
#[derive(Debug, Error)]
pub enum ProjectFromPathError {
    /// An error that occurred while reading the manifest
    #[error("error reading manifest")]
    ManifestRead(#[from] ManifestReadError),

    /// An error that occurred while creating the project
    #[error("error creating project")]
    NewProject(#[from] NewProjectError),
}

impl Project {
    /// Creates a new project
    pub fn new<P: AsRef<Path>, Q: AsRef<Path>>(
        path: P,
        cache_path: Q,
        indices: Indices,
        manifest: Manifest,
    ) -> Result<Self, NewProjectError> {
        if !indices.contains_key(DEFAULT_INDEX_NAME) {
            return Err(NewProjectError::DefaultIndexNotProvided);
        }

        Ok(Self {
            path: path.as_ref().to_path_buf(),
            cache_path: cache_path.as_ref().to_path_buf(),
            indices,
            manifest,
            reqwest_client: reqwest::blocking::ClientBuilder::new()
                .user_agent(concat!(
                    env!("CARGO_PKG_NAME"),
                    "/",
                    env!("CARGO_PKG_VERSION")
                ))
                .build()
                .unwrap(),
        })
    }

    /// Creates a new project from a path (manifest will be read from the path)
    pub fn from_path<P: AsRef<Path>, Q: AsRef<Path>>(
        path: P,
        cache_path: Q,
        indices: Indices,
    ) -> Result<Self, ProjectFromPathError> {
        let manifest = Manifest::from_path(path.as_ref())?;

        Ok(Self::new(path, cache_path, indices, manifest)?)
    }

    /// Returns the indices of the project
    pub fn indices(&self) -> &HashMap<String, Box<dyn Index>> {
        &self.indices
    }

    #[cfg(feature = "wally")]
    pub(crate) fn indices_mut(&mut self) -> &mut HashMap<String, Box<dyn Index>> {
        &mut self.indices
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
    pub fn install(&mut self, install_options: InstallOptions) -> Result<(), InstallProjectError> {
        let map = match install_options.resolved_versions_map {
            Some(map) => map,
            None => {
                let manifest = self.manifest.clone();

                manifest.dependency_tree(self, install_options.locked)?
            }
        };

        if install_options.auto_download {
            self.download(map.clone())?.wait()?;
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
