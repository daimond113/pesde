use std::path::PathBuf;

use clap::Subcommand;

use crate::{cli::CLI_CONFIG, CliConfig};

#[derive(Subcommand, Clone)]
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

pub fn config_command(cmd: ConfigCommand) -> anyhow::Result<()> {
    match cmd {
        ConfigCommand::SetIndexRepo { url } => {
            let cli_config = CliConfig {
                index_repo_url: url.clone(),
                ..CLI_CONFIG.clone()
            };

            cli_config.write()?;

            println!("index repository url set to: `{url}`");
        }
        ConfigCommand::GetIndexRepo => {
            println!(
                "current index repository url: `{}`",
                CLI_CONFIG.index_repo_url
            );
        }
        ConfigCommand::SetCacheDir { directory } => {
            let cli_config = CliConfig {
                cache_dir: directory,
                ..CLI_CONFIG.clone()
            };

            cli_config.write()?;

            println!(
                "cache directory set to: `{}`",
                cli_config.cache_dir().display()
            );
        }
        ConfigCommand::GetCacheDir => {
            println!(
                "current cache directory: `{}`",
                CLI_CONFIG.cache_dir().display()
            );
        }
    }

    Ok(())
}
