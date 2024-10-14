use crate::cli::auth::{get_token_login, get_tokens};
use clap::Args;
use colored::Colorize;

#[derive(Debug, Args)]
pub struct WhoAmICommand {}

impl WhoAmICommand {
    pub fn run(
        self,
        index_url: gix::Url,
        reqwest: reqwest::blocking::Client,
    ) -> anyhow::Result<()> {
        let tokens = get_tokens()?;
        let token = match tokens.0.get(&index_url) {
            Some(token) => token,
            None => {
                println!("not logged in into {index_url}");
                return Ok(());
            }
        };

        println!(
            "logged in as {} into {index_url}",
            get_token_login(&reqwest, token)?.bold()
        );

        Ok(())
    }
}
