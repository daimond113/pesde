use std::{
    collections::BTreeMap,
    fs::{create_dir_all, read},
    hash::{DefaultHasher, Hash, Hasher},
    io::Cursor,
    path::Path,
};

use git2::build::RepoBuilder;
use log::{debug, error};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::{
    dependencies::{maybe_authenticated_request, DependencySpecifier},
    index::{remote_callbacks, IndexFileEntry, WallyIndex},
    manifest::{DependencyType, ManifestConvertError, Realm},
    package_name::{
        FromStrPackageNameParseError, WallyPackageName, WallyPackageNameValidationError,
    },
    project::{get_wally_index, Indices},
};

/// A dependency of a package that can be downloaded from a registry
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct WallyDependencySpecifier {
    /// The name of the package
    #[serde(rename = "wally")]
    pub name: WallyPackageName,
    /// The version requirement of the package
    pub version: VersionReq,
    /// The url of the index
    pub index_url: Url,
    /// The realm of the package
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm: Option<Realm>,
}

/// A reference to a package that can be downloaded from a registry
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct WallyPackageRef {
    /// The name of the package
    pub name: WallyPackageName,
    /// The version of the package
    pub version: Version,
    /// The index URL of the package
    pub index_url: Url,
}

/// An error that occurred while downloading a package from a wally registry
#[derive(Debug, Error)]
pub enum WallyDownloadError {
    /// An error that occurred while interacting with reqwest
    #[error("error interacting with reqwest")]
    Reqwest(#[from] reqwest::Error),

    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// The package was not found on the registry
    #[error("package {0} not found on the registry, but found in the index")]
    NotFound(WallyPackageName),

    /// The user is unauthorized to download the package
    #[error("unauthorized to download package {0}")]
    Unauthorized(WallyPackageName),

    /// An HTTP error occurred
    #[error("http error {0}: the server responded with {1}")]
    Http(reqwest::StatusCode, String),

    /// An error occurred while extracting the archive
    #[error("error extracting archive")]
    Zip(#[from] zip::result::ZipError),

    /// An error occurred while interacting with git
    #[error("error interacting with git")]
    Git(#[from] git2::Error),

    /// An error occurred while interacting with serde
    #[error("error interacting with serde")]
    Serde(#[from] serde_json::Error),

    /// An error occurred while parsing the api URL
    #[error("error parsing URL")]
    Url(#[from] url::ParseError),

    /// An error occurred while refreshing the index
    #[error("error refreshing index")]
    RefreshIndex(#[from] crate::index::RefreshError),

    /// An error occurred while converting the manifest
    #[error("error converting manifest")]
    Manifest(#[from] ManifestConvertError),
}

/// An error that occurred while cloning a wally index
#[derive(Error, Debug)]
pub enum CloneWallyIndexError {
    /// An error that occurred while interacting with git
    #[error("error interacting with git")]
    Git(#[from] git2::Error),

    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while refreshing the index
    #[error("error refreshing index")]
    RefreshIndex(#[from] crate::index::RefreshError),
}

pub(crate) fn clone_wally_index(
    cache_dir: &Path,
    indices: &mut Indices,
    index_url: &Url,
) -> Result<WallyIndex, CloneWallyIndexError> {
    let mut hasher = DefaultHasher::new();
    index_url.hash(&mut hasher);
    let url_hash = hasher.finish().to_string();

    let index_path = cache_dir.join("wally_indices").join(url_hash);

    if index_path.exists() {
        debug!("wally index already exists at {}", index_path.display());

        return Ok(get_wally_index(indices, index_url, Some(&index_path))?.clone());
    }

    debug!(
        "cloning wally index from {} to {}",
        index_url,
        index_path.display()
    );

    create_dir_all(&index_path)?;

    let mut fetch_options = git2::FetchOptions::new();
    fetch_options.remote_callbacks(remote_callbacks!(get_wally_index(
        indices,
        index_url,
        Some(&index_path)
    )?));

    RepoBuilder::new()
        .fetch_options(fetch_options)
        .clone(index_url.as_ref(), &index_path)?;

    Ok(get_wally_index(indices, index_url, Some(&index_path))?.clone())
}

/// The configuration of a wally index
#[derive(Serialize, Deserialize, Debug)]
struct WallyIndexConfig {
    /// The URL of the wally API
    api: String,
}

/// An error that occurred while resolving the URL of a wally package
#[derive(Error, Debug)]
pub enum ResolveWallyUrlError {
    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while interacting with the index
    #[error("error interacting with the index")]
    Index(#[from] crate::index::ConfigError),

    /// An error that occurred while parsing the URL
    #[error("error parsing URL")]
    Url(#[from] url::ParseError),

    /// An error that occurred while cloning the index
    #[error("error cloning index")]
    CloneIndex(#[from] CloneWallyIndexError),

    /// An error that occurred while reading the index config
    #[error("error reading index config")]
    ReadConfig(#[from] serde_json::Error),
}

fn read_api_url(index_path: &Path) -> Result<String, ResolveWallyUrlError> {
    let config_path = index_path.join("config.json");
    let raw_config_contents = read(config_path)?;
    let config: WallyIndexConfig = serde_json::from_slice(&raw_config_contents)?;

    Ok(config.api)
}

impl WallyPackageRef {
    /// Resolves the download URL of the package
    pub fn resolve_url(
        &self,
        cache_dir: &Path,
        indices: &mut Indices,
    ) -> Result<Url, ResolveWallyUrlError> {
        let index = clone_wally_index(cache_dir, indices, &self.index_url)?;

        let api_url = Url::parse(&read_api_url(&index.path)?)?;

        let url = format!(
            "{}/v1/package-contents/{}/{}/{}",
            api_url.to_string().trim_end_matches('/'),
            self.name.scope(),
            self.name.name(),
            self.version
        );

        Ok(Url::parse(&url)?)
    }

    /// Downloads the package to the specified destination
    pub fn download<P: AsRef<Path>>(
        &self,
        reqwest_client: &reqwest::blocking::Client,
        url: &Url,
        registry_auth_token: Option<String>,
        dest: P,
    ) -> Result<(), WallyDownloadError> {
        let response =
            maybe_authenticated_request(reqwest_client, url.as_str(), registry_auth_token)
                .header(
                    "Wally-Version",
                    std::env::var("WALLY_VERSION").unwrap_or("0.3.2".to_string()),
                )
                .send()?;

        if !response.status().is_success() {
            return match response.status() {
                reqwest::StatusCode::NOT_FOUND => {
                    Err(WallyDownloadError::NotFound(self.name.clone()))
                }
                reqwest::StatusCode::UNAUTHORIZED => {
                    Err(WallyDownloadError::Unauthorized(self.name.clone()))
                }
                _ => Err(WallyDownloadError::Http(
                    response.status(),
                    response.text()?,
                )),
            };
        }

        let bytes = response.bytes()?;

        let mut archive = zip::read::ZipArchive::new(Cursor::new(bytes))?;
        archive.extract(dest.as_ref())?;

        Ok(())
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct WallyPackage {
    pub(crate) name: String,
    pub(crate) version: Version,
    pub(crate) registry: Url,
    #[serde(default)]
    pub(crate) realm: Option<String>,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) license: Option<String>,
    #[serde(default)]
    pub(crate) authors: Option<Vec<String>>,
    #[serde(default)]
    pub(crate) private: Option<bool>,
}

#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct WallyPlace {
    #[serde(default)]
    pub(crate) shared_packages: Option<String>,
    #[serde(default)]
    pub(crate) server_packages: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct WallyManifest {
    pub(crate) package: WallyPackage,
    #[serde(default)]
    pub(crate) place: WallyPlace,
    #[serde(default)]
    pub(crate) dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) server_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) dev_dependencies: BTreeMap<String, String>,
}

/// An error that occurred while converting a wally manifest's dependencies
#[derive(Debug, Error)]
pub enum WallyManifestDependencyError {
    /// An error that occurred because the dependency specifier is invalid
    #[error("invalid dependency specifier: {0}")]
    InvalidDependencySpecifier(String),

    /// An error that occurred while parsing a package name
    #[error("error parsing package name")]
    PackageName(#[from] FromStrPackageNameParseError<WallyPackageNameValidationError>),

    /// An error that occurred while parsing a version requirement
    #[error("error parsing version requirement")]
    VersionReq(#[from] semver::Error),
}

pub(crate) fn parse_wally_dependencies(
    manifest: WallyManifest,
) -> Result<BTreeMap<String, DependencySpecifier>, WallyManifestDependencyError> {
    [
        (manifest.dependencies, Realm::Shared),
        (manifest.server_dependencies, Realm::Server),
        (manifest.dev_dependencies, Realm::Development),
    ]
    .into_iter()
    .flat_map(|(deps, realm)| {
        deps.into_iter()
            .map(move |(desired_name, specifier)| (desired_name, specifier, realm))
            .map(|(desired_name, specifier, realm)| {
                let (name, req) = specifier.split_once('@').ok_or_else(|| {
                    WallyManifestDependencyError::InvalidDependencySpecifier(specifier.clone())
                })?;
                let name: WallyPackageName = name.parse()?;
                let req: VersionReq = req.parse()?;

                Ok((
                    desired_name,
                    DependencySpecifier::Wally(WallyDependencySpecifier {
                        name,
                        version: req,
                        index_url: manifest.package.registry.clone(),
                        realm: Some(realm),
                    }),
                ))
            })
    })
    .collect()
}

impl TryFrom<WallyManifest> for IndexFileEntry {
    type Error = WallyManifestDependencyError;

    fn try_from(value: WallyManifest) -> Result<Self, Self::Error> {
        let dependencies = parse_wally_dependencies(value.clone())?
            .into_iter()
            .map(|(desired_name, specifier)| (desired_name, (specifier, DependencyType::Normal)))
            .collect();

        Ok(IndexFileEntry {
            version: value.package.version,
            realm: value
                .package
                .realm
                .map(|r| r.parse().unwrap_or(Realm::Shared)),
            published_at: Default::default(),
            description: value.package.description,
            dependencies,
        })
    }
}
