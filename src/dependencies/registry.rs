use std::path::Path;

use log::{debug, error};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::{
    dependencies::maybe_authenticated_request,
    manifest::Realm,
    package_name::StandardPackageName,
    project::{get_index_by_url, Indices, DEFAULT_INDEX_NAME},
};

fn default_index_name() -> String {
    DEFAULT_INDEX_NAME.to_string()
}

/// A dependency of a package that can be downloaded from a registry
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct RegistryDependencySpecifier {
    /// The name of the package
    pub name: StandardPackageName,
    /// The version requirement of the package
    pub version: VersionReq,
    /// The name of the index to use
    #[serde(default = "default_index_name")]
    pub index: String,
    /// The realm of the package
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm: Option<Realm>,
}

/// A reference to a package that can be downloaded from a registry
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct RegistryPackageRef {
    /// The name of the package
    pub name: StandardPackageName,
    /// The version of the package
    pub version: Version,
    /// The index URL of the package
    pub index_url: Url,
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
    NotFound(StandardPackageName),

    /// The user is unauthorized to download the package
    #[error("unauthorized to download package {0}")]
    Unauthorized(StandardPackageName),

    /// An HTTP error occurred
    #[error("http error {0}: the server responded with {1}")]
    Http(reqwest::StatusCode, String),

    /// An error occurred while parsing the api URL
    #[error("error parsing the API URL")]
    UrlParse(#[from] url::ParseError),
}

/// An error that occurred while resolving the url of a registry package
#[derive(Debug, Error)]
pub enum RegistryUrlResolveError {
    /// An error that occurred while reading the index config
    #[error("error with the index config")]
    IndexConfig(#[from] crate::index::ConfigError),

    /// An error occurred while parsing the api URL
    #[error("error parsing the API URL")]
    UrlParse(#[from] url::ParseError),
}

impl RegistryPackageRef {
    /// Resolves the download URL of the package
    pub fn resolve_url(&self, indices: &Indices) -> Result<Url, RegistryUrlResolveError> {
        let index = get_index_by_url(indices, &self.index_url);
        let config = index.config()?;

        let url = config
            .download()
            .replace("{PACKAGE_AUTHOR}", self.name.scope())
            .replace("{PACKAGE_NAME}", self.name.name())
            .replace("{PACKAGE_VERSION}", &self.version.to_string());

        Ok(Url::parse(&url)?)
    }

    /// Downloads the package to the specified destination
    pub fn download<P: AsRef<Path>>(
        &self,
        reqwest_client: &reqwest::blocking::Client,
        url: &Url,
        registry_auth_token: Option<String>,
        dest: P,
    ) -> Result<(), RegistryDownloadError> {
        debug!(
            "downloading registry package {}@{} from {}",
            self.name, self.version, url
        );

        let response =
            maybe_authenticated_request(reqwest_client, url.as_str(), registry_auth_token)
                .send()?;

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
