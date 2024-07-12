use crate::cli::{auth::get_token_login, get_token, reqwest_client};
use clap::Args;
use colored::Colorize;
use pesde::Project;

#[derive(Debug, Args)]
pub struct WhoAmICommand {}

impl WhoAmICommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        let token = match get_token(project.data_dir())? {
            Some(token) => token,
            None => {
                println!("not logged in");
                return Ok(());
            }
        };

        println!(
            "logged in as {}",
            get_token_login(&reqwest_client(project.data_dir())?, &token)?.bold()
        );

        Ok(())
    }
}
