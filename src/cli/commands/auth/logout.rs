use crate::cli::auth::set_token;
use clap::Args;

#[derive(Debug, Args)]
pub struct LogoutCommand {}

impl LogoutCommand {
    pub fn run(self) -> anyhow::Result<()> {
        set_token(None)?;

        println!("logged out");

        Ok(())
    }
}
