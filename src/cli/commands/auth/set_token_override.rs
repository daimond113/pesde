use crate::cli::config::{read_config, write_config};
use clap::Args;

#[derive(Debug, Args)]
pub struct SetTokenOverrideCommand {
    /// The repository to add the token to
    #[arg(index = 1, value_parser = crate::cli::parse_gix_url)]
    repository: gix::Url,

    /// The token to set
    #[arg(index = 2)]
    token: Option<String>,
}

impl SetTokenOverrideCommand {
    pub fn run(self) -> anyhow::Result<()> {
        let mut config = read_config()?;

        if let Some(token) = self.token {
            println!("set token for {}", self.repository);
            config.token_overrides.insert(self.repository, token);
        } else {
            println!("removed token for {}", self.repository);
            config.token_overrides.remove(&self.repository);
        }

        write_config(&config)?;

        Ok(())
    }
}
