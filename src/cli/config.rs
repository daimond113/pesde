use std::path::PathBuf;

use clap::Subcommand;

use crate::{cli::CLI_CONFIG, CliConfig};

#[derive(Subcommand, Clone)]
pub enum ConfigCommand {
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
        ConfigCommand::SetCacheDir { directory } => {
            let cli_config = CliConfig {
                cache_dir: directory,
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
