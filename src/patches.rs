use crate::{lockfile::DownloadedGraph, Project, MANIFEST_FILE_NAME, PACKAGES_CONTAINER_NAME};
use git2::{ApplyLocation, Diff, DiffFormat, DiffLineType, Repository, Signature};
use std::{fs::read, path::Path};

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

                {
                    let repo = setup_patches_repo(&container_folder)?;
                    repo.apply(&patch, ApplyLocation::Both, None)?;
                }

                std::fs::remove_dir_all(container_folder.join(".git")).map_err(|e| {
                    errors::ApplyPatchesError::GitDirectoryRemovalError(container_folder, e)
                })?;
            }
        }

        Ok(())
    }
}

pub mod errors {
    use std::path::PathBuf;

    use crate::{names::PackageNames, source::VersionId};
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum ApplyPatchesError {
        #[error("error deserializing project manifest")]
        ManifestDeserializationFailed(#[from] crate::errors::ManifestReadError),

        #[error("error interacting with git")]
        GitError(#[from] git2::Error),

        #[error("error reading patch file at {0}")]
        PatchReadError(PathBuf, #[source] std::io::Error),

        #[error("error removing .git directory")]
        GitDirectoryRemovalError(PathBuf, #[source] std::io::Error),

        #[error("package {0}@{1} not found in graph")]
        PackageNotFound(PackageNames, VersionId),
    }
}
