use clap::Subcommand;
use pesde::index::Index;
use reqwest::{header::AUTHORIZATION, Url};

use crate::cli::{api_token::API_TOKEN_SOURCE, send_request, DEFAULT_INDEX, REQWEST_CLIENT};

#[derive(Subcommand, Clone)]
pub enum AuthCommand {
    /// Logs in to the registry
    Login,
    /// Logs out from the registry
    Logout,
}

pub fn auth_command(cmd: AuthCommand) -> anyhow::Result<()> {
    match cmd {
        AuthCommand::Login => {
            let github_oauth_client_id = DEFAULT_INDEX.config()?.github_oauth_client_id;

            let response = send_request(REQWEST_CLIENT.post(Url::parse_with_params(
                "https://github.com/login/device/code",
                &[("client_id", &github_oauth_client_id)],
            )?))?
            .json::<serde_json::Value>()?;

            println!(
                "go to {} and enter the code `{}`",
                response["verification_uri"], response["user_code"]
            );

            let mut time_left = response["expires_in"]
                .as_i64()
                .ok_or(anyhow::anyhow!("couldn't get expires_in"))?;
            let interval = std::time::Duration::from_secs(
                response["interval"]
                    .as_u64()
                    .ok_or(anyhow::anyhow!("couldn't get interval"))?,
            );
            let device_code = response["device_code"]
                .as_str()
                .ok_or(anyhow::anyhow!("couldn't get device_code"))?;

            while time_left > 0 {
                std::thread::sleep(interval);
                time_left -= interval.as_secs() as i64;
                let response = send_request(REQWEST_CLIENT.post(Url::parse_with_params(
                    "https://github.com/login/oauth/access_token",
                    &[
                        ("client_id", github_oauth_client_id.as_str()),
                        ("device_code", device_code),
                        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ],
                )?))?
                .json::<serde_json::Value>()?;

                match response
                    .get("error")
                    .map(|s| {
                        s.as_str()
                            .ok_or(anyhow::anyhow!("couldn't get error as string"))
                    })
                    .unwrap_or(Ok(""))?
                {
                    "authorization_pending" => continue,
                    "slow_down" => {
                        std::thread::sleep(std::time::Duration::from_secs(5));
                        continue;
                    }
                    "expired_token" => {
                        break;
                    }
                    "access_denied" => {
                        anyhow::bail!("access denied, re-run the login command");
                    }
                    _ => (),
                }

                if response.get("access_token").is_some() {
                    let access_token = response["access_token"]
                        .as_str()
                        .ok_or(anyhow::anyhow!("couldn't get access_token"))?;

                    API_TOKEN_SOURCE.set_api_token(access_token)?;

                    let response = send_request(
                        REQWEST_CLIENT
                            .get("https://api.github.com/user")
                            .header(AUTHORIZATION, format!("Bearer {access_token}")),
                    )?
                    .json::<serde_json::Value>()?;

                    let login = response["login"]
                        .as_str()
                        .ok_or(anyhow::anyhow!("couldn't get login"))?;

                    println!("you're now logged in as {login}");
                    return Ok(());
                }
            }

            anyhow::bail!("code expired, please re-run the login command");
        }
        AuthCommand::Logout => {
            API_TOKEN_SOURCE.delete_api_token()?;

            println!("you're now logged out");
        }
    }

    Ok(())
}
