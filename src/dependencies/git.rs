use std::{fs::create_dir_all, path::Path};

use git2::{build::RepoBuilder, Repository};
use log::{debug, warn};
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    index::{remote_callbacks, Index},
    manifest::{Manifest, ManifestConvertError, Realm},
    package_name::PackageName,
    project::Project,
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
    pub name: PackageName,
    /// The version of the package
    pub version: Version,
    /// The URL of the git repository
    pub repo_url: String,
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
}

impl GitDependencySpecifier {
    pub(crate) fn resolve<I: Index>(
        &self,
        project: &Project<I>,
    ) -> Result<(Manifest, String, String), GitDownloadError> {
        debug!("resolving git dependency {}", self.repo);

        // should also work with ssh urls
        let is_url = self.repo.contains(':');

        let repo_name = if !is_url {
            self.repo.to_string()
        } else {
            let parts: Vec<&str> = self.repo.split('/').collect();
            format!(
                "{}/{}",
                parts[parts.len() - 2],
                parts[parts.len() - 1].trim_end_matches(".git")
            )
        };

        if is_url {
            debug!("resolved git repository name to: {}", &repo_name);
        } else {
            debug!("assuming git repository is a name: {}", &repo_name);
        }

        let repo_url = if !is_url {
            format!("https://github.com/{}.git", &self.repo)
        } else {
            self.repo.to_string()
        };

        if is_url {
            debug!("assuming git repository is a url: {}", &repo_url);
        } else {
            debug!("resolved git repository url to: {}", &repo_url);
        }

        let dest = project
            .cache_dir()
            .join("git")
            .join(repo_name.replace('/', "_"))
            .join(&self.rev);

        let repo = if !dest.exists() {
            create_dir_all(&dest)?;

            let mut fetch_options = git2::FetchOptions::new();
            fetch_options.remote_callbacks(remote_callbacks(project.index()));

            RepoBuilder::new()
                .fetch_options(fetch_options)
                .clone(&repo_url, &dest)?
        } else {
            Repository::open(&dest)?
        };

        let obj = repo.revparse_single(&self.rev)?;
        debug!("resolved git revision {} to: {}", self.rev, obj.id());

        repo.reset(&obj, git2::ResetType::Hard, None)?;

        Ok((
            Manifest::from_path_or_convert(dest)?,
            repo_url.to_string(),
            obj.id().to_string(),
        ))
    }
}

impl GitPackageRef {
    /// Downloads the package to the specified destination
    pub fn download<P: AsRef<Path>, I: Index>(
        &self,
        project: &Project<I>,
        dest: P,
    ) -> Result<(), GitDownloadError> {
        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.remote_callbacks(remote_callbacks(project.index()));

        let repo = RepoBuilder::new()
            .fetch_options(fetch_options)
            .clone(&self.repo_url, dest.as_ref())?;

        let obj = repo.revparse_single(&self.rev)?;

        if self.rev != obj.id().to_string() {
            warn!(
                "git package ref {} resolved to a different revision: {}. this shouldn't happen",
                self.rev,
                obj.id()
            );
        }

        repo.reset(&obj, git2::ResetType::Hard, None)?;
        
        Manifest::from_path_or_convert(dest)?;

        Ok(())
    }
}
