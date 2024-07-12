use crate::cli::set_token;
use clap::Args;
use pesde::Project;

#[derive(Debug, Args)]
pub struct LogoutCommand {}

impl LogoutCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        set_token(project.data_dir(), None)?;

        println!("logged out");

        Ok(())
    }
}
