use std::{
    fs::{read, read_dir},
    path::{Path, PathBuf},
};

use git2::{ApplyLocation, Diff, DiffFormat, DiffLineType, Repository, Signature};
use log::debug;
use semver::Version;
use thiserror::Error;

use crate::{
    dependencies::resolution::ResolvedVersionsMap,
    package_name::{FromEscapedStrPackageNameError, PackageName},
    project::Project,
    PATCHES_FOLDER,
};

fn make_signature<'a>() -> Result<Signature<'a>, git2::Error> {
    Signature::now(
        env!("CARGO_PKG_NAME"),
        concat!(env!("CARGO_PKG_NAME"), "@localhost"),
    )
}

/// Sets up a patches repository in the specified directory
pub fn setup_patches_repo<P: AsRef<Path>>(dir: P) -> Result<Repository, git2::Error> {
    let repo = Repository::init(&dir)?;

    {
        let signature = make_signature()?;
        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let oid = index.write_tree()?;
        let tree = repo.find_tree(oid)?;

        repo.commit(Some("HEAD"), &signature, &signature, "original", &tree, &[])?;
    }

    Ok(repo)
}

/// An error that occurred while creating patches
#[derive(Debug, Error)]
pub enum CreatePatchError {
    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while interacting with git
    #[error("error interacting with git")]
    Git(#[from] git2::Error),

    /// An error that occurred while getting a file name
    #[error("failed to get file name from {0}")]
    FileNameFail(PathBuf),

    /// An error that occurred while stripping a prefix
    #[error("error stripping prefix {1} from path {2}")]
    StripPrefixFail(#[source] std::path::StripPrefixError, PathBuf, PathBuf),
}

/// Creates a patch for the package in the specified directory
pub fn create_patch<P: AsRef<Path>>(dir: P) -> Result<Vec<u8>, CreatePatchError> {
    let mut patches = vec![];
    let repo = Repository::open(dir.as_ref())?;

    let original = repo.head()?.peel_to_tree()?;
    let diff = repo.diff_tree_to_workdir(Some(&original), None)?;

    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin_value() {
            DiffLineType::Context | DiffLineType::Addition | DiffLineType::Deletion => {
                let origin = line.origin();
                let mut buffer = vec![0; origin.len_utf8()];
                origin.encode_utf8(&mut buffer);
                patches.extend(buffer);
            }
            _ => {}
        }
        patches.extend(line.content());

        true
    })?;

    Ok(patches)
}

/// An error that occurred while applying patches
#[derive(Debug, Error)]
pub enum ApplyPatchesError {
    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while interacting with git
    #[error("error interacting with git")]
    Git(#[from] git2::Error),

    /// An error that occurred while getting a file name
    #[error("failed to get file name from {0}")]
    FileNameFail(PathBuf),

    /// An error that occurred while converting a path to a string
    #[error("failed to convert path to string")]
    ToStringFail,

    /// An error that occurred because a patch name was malformed
    #[error("malformed patch name {0}")]
    MalformedPatchName(String),

    /// An error that occurred while parsing a package name
    #[error("failed to parse package name {0}")]
    PackageNameParse(#[from] FromEscapedStrPackageNameError),

    /// An error that occurred while getting a file stem
    #[error("failed to get file stem")]
    FileStemFail,

    /// An error that occurred while reading a file
    #[error("failed to read file")]
    ReadFail,

    /// An error that occurred because a package was not found in the dependencies
    #[error("package {0} not found in the lockfile")]
    PackageNotFound(PackageName),

    /// An error that occurred because a version was not found for a package
    #[error("version {0} not found for package {1}")]
    VersionNotFound(Version, PackageName),

    /// An error that occurred while parsing a version
    #[error("failed to parse version")]
    VersionParse(#[from] semver::Error),

    /// An error that occurred while stripping a prefix
    #[error("strip prefix error")]
    StripPrefixFail(#[from] std::path::StripPrefixError),
}

impl Project {
    /// Applies patches for the project
    pub fn apply_patches(&self, map: &ResolvedVersionsMap) -> Result<(), ApplyPatchesError> {
        let patches_dir = self.path().join(PATCHES_FOLDER);
        if !patches_dir.exists() {
            return Ok(());
        }

        for file in read_dir(&patches_dir)? {
            let file = file?;
            if !file.file_type()?.is_file() {
                continue;
            }

            let path = file.path();

            let file_name = path
                .file_name()
                .ok_or_else(|| ApplyPatchesError::FileNameFail(path.clone()))?;
            let file_name = file_name.to_str().ok_or(ApplyPatchesError::ToStringFail)?;

            let (package_name, version) = file_name
                .strip_suffix(".patch")
                .unwrap_or(file_name)
                .split_once('@')
                .ok_or_else(|| ApplyPatchesError::MalformedPatchName(file_name.to_string()))?;

            let package_name = PackageName::from_escaped_str(package_name)?;

            let version = Version::parse(version)?;

            let resolved_pkg = map
                .get(&package_name)
                .ok_or_else(|| ApplyPatchesError::PackageNotFound(package_name.clone()))?
                .get(&version)
                .ok_or_else(|| {
                    ApplyPatchesError::VersionNotFound(version.clone(), package_name.clone())
                })?;

            debug!("resolved package {package_name}@{version} to {resolved_pkg}");

            let (_, source_path) = resolved_pkg.directory(self.path());
            let diff = Diff::from_buffer(&read(&path)?)?;

            let repo = match Repository::open(&source_path) {
                Ok(repo) => repo,
                Err(_) => setup_patches_repo(&source_path)?,
            };

            repo.apply(&diff, ApplyLocation::Both, None)?;

            let mut index = repo.index()?;
            index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
            index.write()?;

            let signature = make_signature()?;
            let tree_id = index.write_tree()?;
            let tree = repo.find_tree(tree_id)?;
            let parent = repo.head()?.peel_to_commit()?;
            repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                "applied patches",
                &tree,
                &[&parent],
            )?;
        }

        Ok(())
    }
}
