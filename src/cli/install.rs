use clap::Args;
use pesde::Project;

#[derive(Debug, Args)]
pub struct InstallCommand {}

impl InstallCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        dbg!(project.dependency_graph(None)?);

        Ok(())
    }
}
