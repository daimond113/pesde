use anyhow::Context;
use clap::Args;
use pesde::{
    names::PackageName,
    scripts::{execute_lune_script, execute_script},
    Project,
};
use relative_path::RelativePathBuf;

#[derive(Debug, Args)]
pub struct RunCommand {
    /// The package name, script name, or path to a script to run
    #[arg(index = 1)]
    package_or_script: String,

    /// Arguments to pass to the script
    #[arg(index = 2, last = true)]
    args: Vec<String>,
}

impl RunCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        if let Ok(_pkg_name) = self.package_or_script.parse::<PackageName>() {
            todo!("implement binary package execution")
        }

        if let Ok(manifest) = project.deser_manifest() {
            if manifest.scripts.contains_key(&self.package_or_script) {
                execute_script(&manifest, &self.package_or_script, &self.args)
                    .context("failed to execute script")?;

                return Ok(());
            }
        };

        let relative_path = RelativePathBuf::from(self.package_or_script);
        let path = relative_path.to_path(project.path());

        if !path.exists() {
            anyhow::bail!("path does not exist: {}", path.display());
        }

        execute_lune_script(None, &relative_path, &self.args)
            .context("failed to execute script")?;

        Ok(())
    }
}
