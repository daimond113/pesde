#![deny(missing_docs)]
//! pesde is a package manager for Luau, designed to be feature-rich and easy to use.
//! pesde has its own registry, however it can also use Wally, and GitHub as package sources.
//! It has been designed with multiple targets in mind, namely Roblox, Lune, and Luau.

#[cfg(not(any(feature = "roblox", feature = "lune", feature = "luau")))]
compile_error!("at least one of the features `roblox`, `lune`, or `luau` must be enabled");

use crate::lockfile::Lockfile;
use gix::sec::identity::Account;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// Downloading packages
pub mod download;
/// Linking packages
pub mod linking;
/// Lockfile
pub mod lockfile;
/// Manifest
pub mod manifest;
/// Package names
pub mod names;
/// Patching packages
#[cfg(feature = "patches")]
pub mod patches;
/// Resolving packages
pub mod resolver;
/// Running scripts
pub mod scripts;
/// Package sources
pub mod source;
pub(crate) mod util;

/// The name of the manifest file
pub const MANIFEST_FILE_NAME: &str = "pesde.toml";
/// The name of the lockfile
pub const LOCKFILE_FILE_NAME: &str = "pesde.lock";
/// The name of the default index
pub const DEFAULT_INDEX_NAME: &str = "default";
/// The name of the packages container
pub const PACKAGES_CONTAINER_NAME: &str = ".pesde";
pub(crate) const LINK_LIB_NO_FILE_FOUND: &str = "____pesde_no_export_file_found";

/// Struct containing the authentication configuration
#[derive(Debug, Default, Clone)]
pub struct AuthConfig {
    default_token: Option<String>,
    token_overrides: HashMap<gix::Url, String>,
    git_credentials: Option<Account>,
}

impl AuthConfig {
    /// Create a new `AuthConfig`
    pub fn new() -> Self {
        AuthConfig::default()
    }

    /// Sets the default token
    pub fn with_default_token<S: AsRef<str>>(mut self, token: Option<S>) -> Self {
        self.default_token = token.map(|s| s.as_ref().to_string());
        self
    }

    /// Set the token overrides
    pub fn with_token_overrides<I: IntoIterator<Item = (gix::Url, S)>, S: AsRef<str>>(
        mut self,
        tokens: I,
    ) -> Self {
        self.token_overrides = tokens
            .into_iter()
            .map(|(url, s)| (url, s.as_ref().to_string()))
            .collect();
        self
    }

    /// Set the git credentials
    pub fn with_git_credentials(mut self, git_credentials: Option<Account>) -> Self {
        self.git_credentials = git_credentials;
        self
    }

    /// Get the default token
    pub fn default_token(&self) -> Option<&str> {
        self.default_token.as_deref()
    }

    /// Get the token overrides
    pub fn token_overrides(&self) -> &HashMap<gix::Url, String> {
        &self.token_overrides
    }

    /// Get the git credentials
    pub fn git_credentials(&self) -> Option<&Account> {
        self.git_credentials.as_ref()
    }

    pub(crate) fn get_token(&self, url: &gix::Url) -> Option<&str> {
        self.token_overrides
            .get(url)
            .map(|s| s.as_str())
            .or(self.default_token.as_deref())
    }
}

/// The main struct of the pesde library, representing a project
#[derive(Debug, Clone)]
pub struct Project {
    path: PathBuf,
    data_dir: PathBuf,
    auth_config: AuthConfig,
    cas_dir: PathBuf,
}

impl Project {
    /// Create a new `Project`
    pub fn new<P: AsRef<Path>, Q: AsRef<Path>, R: AsRef<Path>>(
        path: P,
        data_dir: Q,
        cas_dir: R,
        auth_config: AuthConfig,
    ) -> Self {
        Project {
            path: path.as_ref().to_path_buf(),
            data_dir: data_dir.as_ref().to_path_buf(),
            auth_config,
            cas_dir: cas_dir.as_ref().to_path_buf(),
        }
    }

    /// Access the path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Access the data directory
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Access the authentication configuration
    pub fn auth_config(&self) -> &AuthConfig {
        &self.auth_config
    }

    /// Access the CAS (content-addressable storage) directory
    pub fn cas_dir(&self) -> &Path {
        &self.cas_dir
    }

    /// Read the manifest file
    pub fn read_manifest(&self) -> Result<String, errors::ManifestReadError> {
        let string = std::fs::read_to_string(self.path.join(MANIFEST_FILE_NAME))?;
        Ok(string)
    }

    /// Deserialize the manifest file
    pub fn deser_manifest(&self) -> Result<manifest::Manifest, errors::ManifestReadError> {
        let string = std::fs::read_to_string(self.path.join(MANIFEST_FILE_NAME))?;
        Ok(toml::from_str(&string)?)
    }

    /// Write the manifest file
    pub fn write_manifest<S: AsRef<[u8]>>(&self, manifest: S) -> Result<(), std::io::Error> {
        std::fs::write(self.path.join(MANIFEST_FILE_NAME), manifest.as_ref())
    }

    /// Deserialize the lockfile
    pub fn deser_lockfile(&self) -> Result<Lockfile, errors::LockfileReadError> {
        let string = std::fs::read_to_string(self.path.join(LOCKFILE_FILE_NAME))?;
        Ok(toml::from_str(&string)?)
    }

    /// Write the lockfile
    pub fn write_lockfile(&self, lockfile: Lockfile) -> Result<(), errors::LockfileWriteError> {
        let string = toml::to_string(&lockfile)?;
        std::fs::write(self.path.join(LOCKFILE_FILE_NAME), string)?;
        Ok(())
    }
}

/// Errors that can occur when using the pesde library
pub mod errors {
    use thiserror::Error;

    /// Errors that can occur when reading the manifest file
    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum ManifestReadError {
        /// An IO error occurred
        #[error("io error reading manifest file")]
        Io(#[from] std::io::Error),

        /// An error occurred while deserializing the manifest file
        #[error("error deserializing manifest file")]
        Serde(#[from] toml::de::Error),
    }

    /// Errors that can occur when reading the lockfile
    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum LockfileReadError {
        /// An IO error occurred
        #[error("io error reading lockfile")]
        Io(#[from] std::io::Error),

        /// An error occurred while deserializing the lockfile
        #[error("error deserializing lockfile")]
        Serde(#[from] toml::de::Error),
    }

    /// Errors that can occur when writing the lockfile
    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum LockfileWriteError {
        /// An IO error occurred
        #[error("io error writing lockfile")]
        Io(#[from] std::io::Error),

        /// An error occurred while serializing the lockfile
        #[error("error serializing lockfile")]
        Serde(#[from] toml::ser::Error),
    }
}
