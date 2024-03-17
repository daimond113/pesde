use once_cell::sync::Lazy;

use cli::{auth::auth_command, config::config_command, root::root_command};

use crate::cli::{CliConfig, Command, CLI, MULTI};

mod cli;

fn main() -> anyhow::Result<()> {
    Lazy::force(&MULTI);

    match CLI.command.clone() {
        Command::Auth { command } => auth_command(command),
        Command::Config { command } => config_command(command),
        cmd => root_command(cmd),
    }
}
