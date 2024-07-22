use crate::cli::update_scripts_folder;
use clap::Args;
use pesde::Project;

#[derive(Debug, Args)]
pub struct SelfInstallCommand {}

impl SelfInstallCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        update_scripts_folder(&project)?;

        Ok(())
    }
}
