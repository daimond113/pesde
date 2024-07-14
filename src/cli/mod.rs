use anyhow::Context;
use keyring::Entry;
use pesde::Project;
use serde::{Deserialize, Serialize};
use std::path::Path;

mod auth;
mod config;
mod init;
mod install;
mod run;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub default_index: url::Url,
    pub token: Option<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            default_index: "https://github.com/daimond113/pesde-index".parse().unwrap(),
            token: None,
        }
    }
}

pub fn read_config(data_dir: &Path) -> anyhow::Result<CliConfig> {
    let config_string = match std::fs::read_to_string(data_dir.join("config.yaml")) {
        Ok(config_string) => config_string,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CliConfig::default());
        }
        Err(e) => return Err(e).context("failed to read config file"),
    };

    let config = serde_yaml::from_str(&config_string).context("failed to parse config file")?;

    Ok(config)
}

pub fn write_config(data_dir: &Path, config: &CliConfig) -> anyhow::Result<()> {
    let config_string = serde_yaml::to_string(config).context("failed to serialize config")?;
    std::fs::write(data_dir.join("config.yaml"), config_string)
        .context("failed to write config file")?;

    Ok(())
}

pub fn get_token(data_dir: &Path) -> anyhow::Result<Option<String>> {
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

pub fn set_token(data_dir: &Path, token: Option<&str>) -> anyhow::Result<()> {
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

    let mut config = read_config(data_dir)?;
    config.token = token.map(|s| s.to_string());
    write_config(data_dir, &config)?;

    Ok(())
}

pub fn reqwest_client(data_dir: &Path) -> anyhow::Result<reqwest::blocking::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(token) = get_token(data_dir)? {
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", token)
                .parse()
                .context("failed to create auth header")?,
        );
    }

    headers.insert(
        reqwest::header::ACCEPT,
        "application/json"
            .parse()
            .context("failed to create accept header")?,
    );

    Ok(reqwest::blocking::Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .default_headers(headers)
        .build()?)
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Authentication-related commands
    #[command(subcommand)]
    Auth(auth::AuthCommands),

    /// Configuration-related commands
    #[command(subcommand)]
    Config(config::ConfigCommands),

    /// Initializes a manifest file in the current directory
    Init(init::InitCommand),

    /// Runs a script, an executable package, or a file with Lune
    Run(run::RunCommand),

    /// Installs all dependencies for the project
    Install(install::InstallCommand),
}

impl Subcommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        match self {
            Subcommand::Auth(auth) => auth.run(project),
            Subcommand::Config(config) => config.run(project),
            Subcommand::Init(init) => init.run(project),
            Subcommand::Run(run) => run.run(project),
            Subcommand::Install(install) => install.run(project),
        }
    }
}
