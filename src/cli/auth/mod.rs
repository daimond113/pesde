use anyhow::Context;
use clap::Subcommand;
use pesde::Project;
use serde::Deserialize;

mod login;
mod logout;
mod whoami;

#[derive(Debug, Deserialize)]
struct UserResponse {
    login: String,
}

pub fn get_token_login(
    reqwest: &reqwest::blocking::Client,
    access_token: &str,
) -> anyhow::Result<String> {
    let response = reqwest
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .context("failed to send user request")?
        .json::<UserResponse>()
        .context("failed to parse user response")?;

    Ok(response.login)
}

#[derive(Debug, Subcommand)]
pub enum AuthCommands {
    /// Logs in into GitHub, and stores the token
    Login(login::LoginCommand),
    /// Removes the stored token
    Logout(logout::LogoutCommand),
    /// Prints the username of the currently logged-in user
    #[clap(name = "whoami")]
    WhoAmI(whoami::WhoAmICommand),
}

impl AuthCommands {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        match self {
            AuthCommands::Login(login) => login.run(project),
            AuthCommands::Logout(logout) => logout.run(project),
            AuthCommands::WhoAmI(whoami) => whoami.run(project),
        }
    }
}
