use crate::cli::{auth::get_token_login, get_token};
use clap::Args;
use colored::Colorize;
use pesde::Project;

#[derive(Debug, Args)]
pub struct WhoAmICommand {}

impl WhoAmICommand {
    pub fn run(self, project: Project, reqwest: reqwest::blocking::Client) -> anyhow::Result<()> {
        let token = match get_token(project.data_dir())? {
            Some(token) => token,
            None => {
                println!("not logged in");
                return Ok(());
            }
        };

        println!("logged in as {}", get_token_login(&reqwest, &token)?.bold());

        Ok(())
    }
}
