use crate::{lockfile::DownloadedGraph, Project, MANIFEST_FILE_NAME, PACKAGES_CONTAINER_NAME};
use git2::{ApplyLocation, ApplyOptions, Diff, DiffFormat, DiffLineType, Repository, Signature};
use relative_path::RelativePathBuf;
use std::{fs::read, path::Path};

/// Set up a git repository for patches
pub fn setup_patches_repo<P: AsRef<Path>>(dir: P) -> Result<Repository, git2::Error> {
    let repo = Repository::init(&dir)?;

    {
        let signature = Signature::now(
            env!("CARGO_PKG_NAME"),
            concat!(env!("CARGO_PKG_NAME"), "@localhost"),
        )?;
        let mut index = repo.index()?;
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let oid = index.write_tree()?;
        let tree = repo.find_tree(oid)?;

        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "begin patch",
            &tree,
            &[],
        )?;
    }

    Ok(repo)
}

/// Create a patch from the current state of the repository
pub fn create_patch<P: AsRef<Path>>(dir: P) -> Result<Vec<u8>, git2::Error> {
    let mut patches = vec![];
    let repo = Repository::open(dir.as_ref())?;

    let original = repo.head()?.peel_to_tree()?;

    // reset the manifest file to the original state
    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.force();
    checkout_builder.path(MANIFEST_FILE_NAME);
    repo.checkout_tree(original.as_object(), Some(&mut checkout_builder))?;

    let diff = repo.diff_tree_to_workdir(Some(&original), None)?;

    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        if matches!(
            line.origin_value(),
            DiffLineType::Context | DiffLineType::Addition | DiffLineType::Deletion
        ) {
            let origin = line.origin();
            let mut buffer = vec![0; origin.len_utf8()];
            origin.encode_utf8(&mut buffer);
            patches.extend(buffer);
        }

        patches.extend(line.content());

        true
    })?;

    Ok(patches)
}

impl Project {
    /// Apply patches to the project's dependencies
    pub fn apply_patches(&self, graph: &DownloadedGraph) -> Result<(), errors::ApplyPatchesError> {
        let manifest = self.deser_manifest()?;

        for (name, versions) in manifest.patches {
            for (version_id, patch_path) in versions {
                let patch_path = patch_path.to_path(self.path());
                let patch = Diff::from_buffer(&read(&patch_path).map_err(|e| {
                    errors::ApplyPatchesError::PatchReadError(patch_path.clone(), e)
                })?)?;

                let Some(node) = graph
                    .get(&name)
                    .and_then(|versions| versions.get(&version_id))
                else {
                    return Err(errors::ApplyPatchesError::PackageNotFound(name, version_id));
                };

                let container_folder = node.node.container_folder(
                    &self
                        .path()
                        .join(node.node.base_folder(manifest.target.kind(), true))
                        .join(PACKAGES_CONTAINER_NAME),
                    &name,
                    version_id.version(),
                );

                log::debug!("applying patch to {name}@{version_id}");

                {
                    let repo = setup_patches_repo(&container_folder)?;
                    let mut apply_opts = ApplyOptions::new();
                    apply_opts.delta_callback(|delta| {
                        let Some(delta) = delta else {
                            return true;
                        };

                        if !matches!(delta.status(), git2::Delta::Modified) {
                            return true;
                        }

                        let file = delta.new_file();
                        let Some(relative_path) = file.path() else {
                            return true;
                        };

                        let relative_path = RelativePathBuf::from_path(relative_path).unwrap();
                        let path = relative_path.to_path(&container_folder);

                        if !path.is_file() {
                            return true;
                        }

                        // there is no way (as far as I know) to check if it's hardlinked
                        // so, we always unlink it
                        let content = read(&path).unwrap();
                        std::fs::remove_file(&path).unwrap();
                        std::fs::write(path, content).unwrap();

                        true
                    });
                    repo.apply(&patch, ApplyLocation::Both, Some(&mut apply_opts))?;
                }

                log::debug!("patch applied to {name}@{version_id}, removing .git directory");

                std::fs::remove_dir_all(container_folder.join(".git")).map_err(|e| {
                    errors::ApplyPatchesError::GitDirectoryRemovalError(container_folder, e)
                })?;
            }
        }

        Ok(())
    }
}

/// Errors that can occur when using patches
pub mod errors {
    use std::path::PathBuf;

    use crate::{names::PackageNames, source::version_id::VersionId};
    use thiserror::Error;

    /// Errors that can occur when applying patches
    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum ApplyPatchesError {
        /// Error deserializing the project manifest
        #[error("error deserializing project manifest")]
        ManifestDeserializationFailed(#[from] crate::errors::ManifestReadError),

        /// Error interacting with git
        #[error("error interacting with git")]
        GitError(#[from] git2::Error),

        /// Error reading the patch file
        #[error("error reading patch file at {0}")]
        PatchReadError(PathBuf, #[source] std::io::Error),

        /// Error removing the .git directory
        #[error("error removing .git directory")]
        GitDirectoryRemovalError(PathBuf, #[source] std::io::Error),

        /// Package not found in the graph
        #[error("package {0}@{1} not found in graph")]
        PackageNotFound(PackageNames, VersionId),
    }
}
