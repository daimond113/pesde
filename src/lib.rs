// #![deny(missing_docs)] - TODO: bring this back before publishing 0.5

#[cfg(not(any(feature = "roblox", feature = "lune", feature = "luau")))]
compile_error!("at least one of the features `roblox`, `lune`, or `luau` must be enabled");

use crate::lockfile::Lockfile;
use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};

pub mod download;
mod git;
pub mod linking;
pub mod lockfile;
pub mod manifest;
pub mod names;
pub mod resolver;
pub mod scripts;
pub mod source;

pub const MANIFEST_FILE_NAME: &str = "pesde.yaml";
pub const LOCKFILE_FILE_NAME: &str = "pesde.lock";
pub const DEFAULT_INDEX_NAME: &str = "default";
pub const PACKAGES_CONTAINER_NAME: &str = ".pesde";
pub const MAX_ARCHIVE_SIZE: usize = 4 * 1024 * 1024;

pub(crate) static REQWEST_CLIENT: Lazy<reqwest::blocking::Client> = Lazy::new(|| {
    reqwest::blocking::Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .expect("failed to create reqwest client")
});

#[derive(Debug, Default, Clone)]
pub struct AuthConfig {
    pesde_token: Option<String>,
    git_credentials: Option<gix::sec::identity::Account>,
}

impl AuthConfig {
    pub fn new() -> Self {
        AuthConfig::default()
    }

    pub fn pesde_token(&self) -> Option<&str> {
        self.pesde_token.as_deref()
    }

    pub fn git_credentials(&self) -> Option<&gix::sec::identity::Account> {
        self.git_credentials.as_ref()
    }

    pub fn with_pesde_token<S: AsRef<str>>(mut self, token: Option<S>) -> Self {
        self.pesde_token = token.map(|s| s.as_ref().to_string());
        self
    }

    pub fn with_git_credentials(
        mut self,
        git_credentials: Option<gix::sec::identity::Account>,
    ) -> Self {
        self.git_credentials = git_credentials;
        self
    }
}

#[derive(Debug)]
pub struct Project {
    path: PathBuf,
    data_dir: PathBuf,
    auth_config: AuthConfig,
}

impl Project {
    pub fn new<P: AsRef<Path>, Q: AsRef<Path>>(
        path: P,
        data_dir: Q,
        auth_config: AuthConfig,
    ) -> Self {
        Project {
            path: path.as_ref().to_path_buf(),
            data_dir: data_dir.as_ref().to_path_buf(),
            auth_config,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn auth_config(&self) -> &AuthConfig {
        &self.auth_config
    }

    pub fn read_manifest(&self) -> Result<Vec<u8>, errors::ManifestReadError> {
        let bytes = std::fs::read(self.path.join(MANIFEST_FILE_NAME))?;
        Ok(bytes)
    }

    pub fn deser_manifest(&self) -> Result<manifest::Manifest, errors::ManifestReadError> {
        let bytes = std::fs::read(self.path.join(MANIFEST_FILE_NAME))?;
        Ok(serde_yaml::from_slice(&bytes)?)
    }

    pub fn write_manifest<S: AsRef<[u8]>>(&self, manifest: S) -> Result<(), std::io::Error> {
        std::fs::write(self.path.join(MANIFEST_FILE_NAME), manifest.as_ref())
    }

    pub fn deser_lockfile(&self) -> Result<Lockfile, errors::LockfileReadError> {
        let bytes = std::fs::read(self.path.join(LOCKFILE_FILE_NAME))?;
        Ok(serde_yaml::from_slice(&bytes)?)
    }

    pub fn write_lockfile(&self, lockfile: Lockfile) -> Result<(), errors::LockfileWriteError> {
        let writer = std::fs::File::create(self.path.join(LOCKFILE_FILE_NAME))?;
        serde_yaml::to_writer(writer, &lockfile)?;
        Ok(())
    }
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum ManifestReadError {
        #[error("io error reading manifest file")]
        Io(#[from] std::io::Error),

        #[error("error deserializing manifest file")]
        Serde(#[from] serde_yaml::Error),
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum LockfileReadError {
        #[error("io error reading lockfile file")]
        Io(#[from] std::io::Error),

        #[error("error deserializing lockfile file")]
        Serde(#[from] serde_yaml::Error),
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum LockfileWriteError {
        #[error("io error writing lockfile file")]
        Io(#[from] std::io::Error),

        #[error("error serializing lockfile file")]
        Serde(#[from] serde_yaml::Error),
    }
}
