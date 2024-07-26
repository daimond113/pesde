use crate::cli::config::{read_config, write_config};
use anyhow::Context;
use keyring::Entry;
use serde::Deserialize;
use std::path::Path;

pub fn get_token<P: AsRef<Path>>(data_dir: P) -> anyhow::Result<Option<String>> {
    match std::env::var("PESDE_TOKEN") {
        Ok(token) => return Ok(Some(token)),
        Err(std::env::VarError::NotPresent) => {}
        Err(e) => return Err(e.into()),
    }

    let config = read_config(data_dir)?;
    if let Some(token) = config.token {
        return Ok(Some(token));
    }

    match Entry::new("token", env!("CARGO_PKG_NAME")) {
        Ok(entry) => match entry.get_password() {
            Ok(token) => return Ok(Some(token)),
            Err(keyring::Error::PlatformFailure(_) | keyring::Error::NoEntry) => {}
            Err(e) => return Err(e.into()),
        },
        Err(keyring::Error::PlatformFailure(_)) => {}
        Err(e) => return Err(e.into()),
    }

    Ok(None)
}

pub fn set_token<P: AsRef<Path>>(data_dir: P, token: Option<&str>) -> anyhow::Result<()> {
    let entry = match Entry::new("token", env!("CARGO_PKG_NAME")) {
        Ok(entry) => entry,
        Err(e) => return Err(e.into()),
    };

    let result = if let Some(token) = token {
        entry.set_password(token)
    } else {
        entry.delete_credential()
    };

    match result {
        Ok(()) => return Ok(()),
        Err(keyring::Error::PlatformFailure(_) | keyring::Error::NoEntry) => {}
        Err(e) => return Err(e.into()),
    }

    let mut config = read_config(&data_dir)?;
    config.token = token.map(|s| s.to_string());
    write_config(data_dir, &config)?;

    Ok(())
}

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
