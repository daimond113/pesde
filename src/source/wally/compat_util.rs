use std::path::Path;

use relative_path::RelativePathBuf;
use serde::Deserialize;
use tempfile::TempDir;

use crate::{
    manifest::target::Target,
    scripts::{execute_script, ScriptName},
    Project, LINK_LIB_NO_FILE_FOUND,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourcemapNode {
    #[serde(default)]
    file_paths: Vec<RelativePathBuf>,
}

pub(crate) fn find_lib_path(
    project: &Project,
    package_dir: &Path,
) -> Result<Option<RelativePathBuf>, errors::FindLibPathError> {
    let manifest = project.deser_manifest()?;

    let Some(script_path) = manifest
        .scripts
        .get(&ScriptName::SourcemapGenerator.to_string())
    else {
        log::warn!("no sourcemap generator script found in manifest");
        return Ok(None);
    };

    let result = execute_script(
        ScriptName::SourcemapGenerator,
        &script_path.to_path(&project.path),
        [package_dir],
        project,
        true,
    )?;

    if let Some(result) = result.filter(|result| !result.is_empty()) {
        let node: SourcemapNode = serde_json::from_str(&result)?;
        Ok(node.file_paths.into_iter().find(|path| {
            path.extension()
                .is_some_and(|ext| ext == "lua" || ext == "luau")
        }))
    } else {
        Ok(None)
    }
}

pub(crate) fn get_target(
    project: &Project,
    tempdir: &TempDir,
) -> Result<Target, errors::FindLibPathError> {
    Ok(Target::Roblox {
        lib: find_lib_path(project, tempdir.path())?
            .or_else(|| Some(RelativePathBuf::from(LINK_LIB_NO_FILE_FOUND))),
        build_files: Default::default(),
    })
}

pub mod errors {
    use thiserror::Error;

    /// Errors that can occur when finding the lib path
    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum FindLibPathError {
        /// An error occurred deserializing the project manifest
        #[error("error deserializing manifest")]
        Manifest(#[from] crate::errors::ManifestReadError),

        /// An error occurred while executing the sourcemap generator script
        #[error("error executing sourcemap generator script")]
        Script(#[from] std::io::Error),

        /// An error occurred while deserializing the sourcemap result
        #[error("error deserializing sourcemap result")]
        Serde(#[from] serde_json::Error),
    }
}
