use std::path::Path;

use log::{debug, error};
use reqwest::header::{AUTHORIZATION, USER_AGENT as USER_AGENT_HEADER};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    index::Index, manifest::Realm, package_name::PackageName, project::Project, USER_AGENT,
};

/// A dependency of a package that can be downloaded from a registry
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct RegistryDependencySpecifier {
    /// The name of the package
    pub name: PackageName,
    /// The version requirement of the package
    pub version: VersionReq,
    // TODO: support per-package registries
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub registry: Option<String>,
    /// The realm of the package
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm: Option<Realm>,
}

/// A reference to a package that can be downloaded from a registry
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct RegistryPackageRef {
    /// The name of the package
    pub name: PackageName,
    /// The version of the package
    pub version: Version,
    // TODO: support per-package registries
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub index_url: Option<String>,
}

/// An error that occurred while downloading a package from a registry
#[derive(Debug, Error)]
pub enum RegistryDownloadError {
    /// An error that occurred while interacting with reqwest
    #[error("error interacting with reqwest")]
    Reqwest(#[from] reqwest::Error),

    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while reading the index config
    #[error("error with the index config")]
    IndexConfig(#[from] crate::index::ConfigError),

    /// The package was not found on the registry
    #[error("package {0} not found on the registry, but found in the index")]
    NotFound(PackageName),

    /// The user is unauthorized to download the package
    #[error("unauthorized to download package {0}")]
    Unauthorized(PackageName),

    /// An HTTP error occurred
    #[error("http error {0}: the server responded with {1}")]
    Http(reqwest::StatusCode, String),
}

impl RegistryPackageRef {
    /// Downloads the package to the specified destination
    pub fn download<P: AsRef<Path>, I: Index>(
        &self,
        project: &Project<I>,
        dest: P,
    ) -> Result<(), RegistryDownloadError> {
        let url = project
            .index()
            .config()?
            .download()
            .replace("{PACKAGE_AUTHOR}", self.name.scope())
            .replace("{PACKAGE_NAME}", self.name.name())
            .replace("{PACKAGE_VERSION}", &self.version.to_string());

        debug!(
            "downloading registry package {}@{} from {}",
            self.name, self.version, url
        );

        let client = reqwest::blocking::Client::new();
        let response = {
            let mut builder = client.get(&url).header(USER_AGENT_HEADER, USER_AGENT);
            if let Some(token) = project.registry_auth_token() {
                let visible_tokens = token.chars().take(8).collect::<String>();
                let hidden_tokens = "*".repeat(token.len() - 8);
                debug!("using registry token {visible_tokens}{hidden_tokens}");
                builder = builder.header(AUTHORIZATION, format!("Bearer {}", token));
            }
            builder.send()?
        };

        if !response.status().is_success() {
            return match response.status() {
                reqwest::StatusCode::NOT_FOUND => {
                    Err(RegistryDownloadError::NotFound(self.name.clone()))
                }
                reqwest::StatusCode::UNAUTHORIZED => {
                    Err(RegistryDownloadError::Unauthorized(self.name.clone()))
                }
                _ => Err(RegistryDownloadError::Http(
                    response.status(),
                    response.text()?,
                )),
            };
        }

        let bytes = response.bytes()?;

        let mut decoder = flate2::read::GzDecoder::new(bytes.as_ref());
        let mut archive = tar::Archive::new(&mut decoder);

        archive.unpack(&dest)?;

        Ok(())
    }
}
