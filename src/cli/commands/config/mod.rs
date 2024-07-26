use clap::Subcommand;
use pesde::Project;

mod default_index;
mod scripts_repo;

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    /// Configuration for the default index
    DefaultIndex(default_index::DefaultIndexCommand),

    /// Configuration for the scripts repository
    ScriptsRepo(scripts_repo::ScriptsRepoCommand),
}

impl ConfigCommands {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        match self {
            ConfigCommands::DefaultIndex(default_index) => default_index.run(project),
            ConfigCommands::ScriptsRepo(scripts_repo) => scripts_repo.run(project),
        }
    }
}
