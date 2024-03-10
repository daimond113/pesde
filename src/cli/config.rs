use std::path::PathBuf;

use clap::Subcommand;

use crate::{CliConfig, CliParams};

#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Sets the index repository URL
    SetIndexRepo {
        /// The URL of the index repository
        #[clap(value_name = "URL")]
        url: String,
    },
    /// Gets the index repository URL
    GetIndexRepo,

    /// Sets the cache directory
    SetCacheDir {
        /// The directory to use as the cache directory
        #[clap(value_name = "DIRECTORY")]
        directory: Option<PathBuf>,
    },
    /// Gets the cache directory
    GetCacheDir,
}

pub fn config_command(cmd: ConfigCommand, params: CliParams) -> anyhow::Result<()> {
    match cmd {
        ConfigCommand::SetIndexRepo { url } => {
            let cli_config = CliConfig {
                index_repo_url: url.clone(),
                ..params.cli_config
            };

            cli_config.write(&params.directories)?;

            println!("index repository url set to: `{url}`");
        }
        ConfigCommand::GetIndexRepo => {
            println!(
                "current index repository url: `{}`",
                params.cli_config.index_repo_url
            );
        }
        ConfigCommand::SetCacheDir { directory } => {
            let cli_config = CliConfig {
                cache_dir: directory,
                ..params.cli_config
            };

            cli_config.write(&params.directories)?;

            println!(
                "cache directory set to: `{}`",
                cli_config.cache_dir(&params.directories).display()
            );
        }
        ConfigCommand::GetCacheDir => {
            println!(
                "current cache directory: `{}`",
                params.cli_config.cache_dir(&params.directories).display()
            );
        }
    }

    Ok(())
}
