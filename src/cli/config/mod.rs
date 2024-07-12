use clap::Subcommand;
use pesde::Project;

mod default_index;

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    /// Configuration for the default index
    DefaultIndex(default_index::DefaultIndexCommand),
}

impl ConfigCommands {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        match self {
            ConfigCommands::DefaultIndex(default_index) => default_index.run(project),
        }
    }
}
