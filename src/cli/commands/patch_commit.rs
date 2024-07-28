use crate::cli::IsUpToDate;
use anyhow::Context;
use clap::Args;
use pesde::{
    manifest::Manifest, names::PackageNames, patches::create_patch, source::version_id::VersionId,
    Project, MANIFEST_FILE_NAME,
};
use std::{path::PathBuf, str::FromStr};

#[derive(Debug, Args)]
pub struct PatchCommitCommand {
    /// The directory containing the patch to commit
    #[arg(index = 1)]
    directory: PathBuf,
}

impl PatchCommitCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        let graph = if project.is_up_to_date(true)? {
            project.deser_lockfile()?.graph
        } else {
            anyhow::bail!("outdated lockfile, please run the install command first")
        };

        let (name, version_id) = {
            let patched_manifest = std::fs::read_to_string(self.directory.join(MANIFEST_FILE_NAME))
                .context("failed to read patched manifest")?;
            let patched_manifest: Manifest =
                toml::from_str(&patched_manifest).context("failed to parse patched manifest")?;

            (
                PackageNames::Pesde(patched_manifest.name),
                VersionId::new(patched_manifest.version, patched_manifest.target.kind()),
            )
        };

        graph
            .get(&name)
            .and_then(|versions| versions.get(&version_id))
            .context("package not found in graph")?;

        let mut manifest = toml_edit::DocumentMut::from_str(
            &project.read_manifest().context("failed to read manifest")?,
        )
        .context("failed to parse manifest")?;

        let patch = create_patch(&self.directory).context("failed to create patch")?;
        std::fs::remove_dir_all(self.directory).context("failed to remove patch directory")?;

        let patches_dir = project.path().join("patches");
        std::fs::create_dir_all(&patches_dir).context("failed to create patches directory")?;

        let patch_file_name = format!("{}-{}.patch", name.escaped(), version_id.escaped(),);

        let patch_file = patches_dir.join(&patch_file_name);
        if patch_file.exists() {
            anyhow::bail!("patch file already exists: {}", patch_file.display());
        }

        std::fs::write(&patch_file, patch).context("failed to write patch file")?;

        manifest["patches"][&name.to_string()][&version_id.to_string()] =
            toml_edit::value(format!("patches/{patch_file_name}"));

        project
            .write_manifest(manifest.to_string())
            .context("failed to write manifest")?;

        println!(concat!(
            "done! run `",
            env!("CARGO_BIN_NAME"),
            " install` to apply the patch"
        ));

        Ok(())
    }
}
