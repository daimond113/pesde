use std::{
    fs::create_dir_all,
    hash::{DefaultHasher, Hash, Hasher},
    path::Path,
    sync::Arc,
};

use git2::{build::RepoBuilder, Repository};
use log::{debug, error, warn};
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::{
    index::{remote_callbacks, CredentialsFn},
    manifest::{update_sync_tool_files, Manifest, ManifestConvertError, Realm},
    package_name::StandardPackageName,
    project::{get_index, Indices},
};

/// A dependency of a package that can be downloaded from a git repository
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct GitDependencySpecifier {
    /// The URL of the git repository (can be in the form of `owner/repo`, in which case it will default to GitHub)
    pub repo: String,
    /// The revision of the git repository to use
    pub rev: String,
    /// The realm of the package
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realm: Option<Realm>,
}

/// A reference to a package that can be downloaded from a git repository
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct GitPackageRef {
    /// The name of the package
    pub name: StandardPackageName,
    /// The version of the package
    pub version: Version,
    /// The URL of the git repository
    pub repo_url: Url,
    /// The revision of the git repository to use
    pub rev: String,
}

/// An error that occurred while downloading a git repository
#[derive(Debug, Error)]
pub enum GitDownloadError {
    /// An error that occurred while interacting with git
    #[error("error interacting with git")]
    Git(#[from] git2::Error),

    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while reading the manifest of the git repository
    #[error("error reading manifest")]
    ManifestRead(#[from] ManifestConvertError),

    /// An error that occurred because the URL is invalid
    #[error("invalid URL")]
    InvalidUrl(#[from] url::ParseError),

    /// An error that occurred while resolving a git dependency's manifest
    #[error("error resolving git dependency manifest")]
    Resolve(#[from] GitManifestResolveError),
}

/// An error that occurred while resolving a git dependency's manifest
#[derive(Debug, Error)]
pub enum GitManifestResolveError {
    /// An error that occurred because the scope and name could not be extracted from the URL
    #[error("could not extract scope and name from URL: {0}")]
    ScopeAndNameFromUrl(Url),

    /// An error that occurred because the package name is invalid
    #[error("invalid package name")]
    InvalidPackageName(#[from] crate::package_name::StandardPackageNameValidationError),

    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),
}

fn to_snake_case(s: &str) -> String {
    s.chars()
        .enumerate()
        .map(|(i, c)| {
            if c.is_uppercase() {
                format!("{}{}", if i == 0 { "" } else { "_" }, c.to_lowercase())
            } else if c == '-' {
                "_".to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

pub(crate) fn manifest(path: &Path, url: &Url) -> Result<Manifest, GitManifestResolveError> {
    Manifest::from_path_or_convert(path).or_else(|_| {
        let (scope, name) = url
            .path_segments()
            .and_then(|mut s| {
                let scope = s.next();
                let name = s.next();

                if let (Some(scope), Some(name)) = (scope, name) {
                    Some((scope.to_string(), name.to_string()))
                } else {
                    None
                }
            })
            .ok_or_else(|| GitManifestResolveError::ScopeAndNameFromUrl(url.clone()))?;

        let manifest = Manifest {
            name: StandardPackageName::new(
                &to_snake_case(&scope),
                &to_snake_case(name.trim_end_matches(".git")),
            )?,
            version: Version::new(0, 1, 0),
            description: None,
            license: None,
            authors: None,
            repository: None,
            exports: Default::default(),
            path_style: Default::default(),
            private: true,
            realm: None,
            indices: Default::default(),
            #[cfg(feature = "wally")]
            sourcemap_generator: None,
            overrides: Default::default(),

            dependencies: Default::default(),
            peer_dependencies: Default::default(),
        };

        manifest.write(path).unwrap();

        update_sync_tool_files(path, manifest.name.name().to_string())?;

        Ok(manifest)
    })
}

impl GitDependencySpecifier {
    pub(crate) fn resolve(
        &self,
        cache_dir: &Path,
        indices: &Indices,
    ) -> Result<(Manifest, Url, String), GitDownloadError> {
        debug!("resolving git dependency {}", self.repo);

        // should also work with ssh urls
        let repo_url = if self.repo.contains(':') {
            debug!("resolved git repository name to: {}", self.repo);
            Url::parse(&self.repo)
        } else {
            debug!("assuming git repository is a name: {}", self.repo);
            Url::parse(&format!("https://github.com/{}.git", &self.repo))
        }?;

        debug!("resolved git repository url to: {}", &repo_url);

        let mut hasher = DefaultHasher::new();
        repo_url.hash(&mut hasher);
        self.rev.hash(&mut hasher);
        let repo_hash = hasher.finish();

        let dest = cache_dir.join("git").join(repo_hash.to_string());

        let repo = if !dest.exists() {
            create_dir_all(&dest)?;

            let mut fetch_options = git2::FetchOptions::new();
            fetch_options.remote_callbacks(remote_callbacks!(get_index(indices, None)));

            RepoBuilder::new()
                .fetch_options(fetch_options)
                .clone(repo_url.as_ref(), &dest)?
        } else {
            Repository::open(&dest)?
        };

        let obj = repo.revparse_single(&self.rev)?;
        debug!("resolved git revision {} to: {}", self.rev, obj.id());

        repo.reset(&obj, git2::ResetType::Hard, None)?;

        Ok((manifest(&dest, &repo_url)?, repo_url, obj.id().to_string()))
    }
}

impl GitPackageRef {
    /// Downloads the package to the specified destination
    pub fn download<P: AsRef<Path>>(
        &self,
        dest: P,
        credentials_fn: Option<Arc<CredentialsFn>>,
    ) -> Result<(), GitDownloadError> {
        let mut fetch_options = git2::FetchOptions::new();
        let mut remote_callbacks = git2::RemoteCallbacks::new();
        let credentials_fn = credentials_fn.map(|f| f());

        if let Some(credentials_fn) = credentials_fn {
            debug!("authenticating this git clone with credentials");
            remote_callbacks.credentials(credentials_fn);
        } else {
            debug!("no credentials provided for this git clone");
        }

        fetch_options.remote_callbacks(remote_callbacks);

        let repo = RepoBuilder::new()
            .fetch_options(fetch_options)
            .clone(self.repo_url.as_ref(), dest.as_ref())?;

        let obj = repo.revparse_single(&self.rev)?;

        if self.rev != obj.id().to_string() {
            warn!(
                "git package ref {} resolved to a different revision: {}. this shouldn't happen",
                self.rev,
                obj.id()
            );
        }

        repo.reset(&obj, git2::ResetType::Hard, None)?;

        Ok(())
    }
}
