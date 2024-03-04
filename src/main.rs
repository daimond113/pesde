use anyhow::bail;
use std::{
    fs::{create_dir_all, read},
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
};

use auth_git2::GitAuthenticator;
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use keyring::Entry;
use pretty_env_logger::env_logger::Env;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};

use cli::{
    auth::{auth_command, AuthCommand},
    config::{config_command, ConfigCommand},
    root::root_command,
};
use pesde::index::GitIndex;

mod cli;

#[derive(Subcommand)]
pub enum Command {
    /// Installs the dependencies of the project
    Install {
        #[clap(long, short)]
        locked: bool,
    },

    /// Runs the `bin` export of the specified package
    Run {
        #[clap(value_name = "PACKAGE")]
        package: String,

        #[clap(last = true)]
        args: Vec<String>,
    },

    /// Searches for a package on the registry
    Search {
        #[clap(value_name = "QUERY")]
        query: Option<String>,
    },

    /// Publishes the project to the registry
    Publish,

    /// Begins a new patch
    Patch { package: String },

    /// Commits (finished) the patch
    PatchCommit {
        #[clap(value_name = "DIRECTORY")]
        dir: PathBuf,
    },

    /// Auth-related commands
    Auth {
        #[clap(subcommand)]
        command: AuthCommand,
    },

    /// Config-related commands
    Config {
        #[clap(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Command,

    #[arg(short, long, value_name = "DIRECTORY")]
    directory: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone)]
struct CliConfig {
    index_repo_url: String,
    cache_dir: Option<PathBuf>,
}

impl CliConfig {
    fn cache_dir(&self, directories: &ProjectDirs) -> PathBuf {
        self.cache_dir
            .clone()
            .unwrap_or_else(|| directories.cache_dir().to_path_buf())
    }
}

struct CliParams {
    index: GitIndex,
    api_token_entry: Entry,
    reqwest_client: reqwest::blocking::Client,
    cli_config: CliConfig,
    cwd: PathBuf,
    multi: MultiProgress,
    directories: ProjectDirs,
}

impl CliConfig {
    fn write(&self, directories: &ProjectDirs) -> anyhow::Result<()> {
        let cli_config_path = directories.config_dir().join("config.yaml");
        serde_yaml::to_writer(
            &mut std::fs::File::create(cli_config_path.as_path())?,
            &self,
        )?;

        Ok(())
    }
}

#[macro_export]
macro_rules! send_request {
    ($req:expr) => {{
        let res = $req.send()?;

        match res.error_for_status_ref() {
            Ok(_) => res,
            Err(e) => {
                panic!("request failed: {e}\nbody: {}", res.text()?);
            }
        }
    }};
}

fn main() -> anyhow::Result<()> {
    let logger = pretty_env_logger::formatted_builder()
        .parse_env(Env::default().default_filter_or("info"))
        .build();
    let multi = MultiProgress::new();

    LogWrapper::new(multi.clone(), logger).try_init().unwrap();

    let cli = Cli::parse();

    let directories = ProjectDirs::from("com", env!("CARGO_BIN_NAME"), env!("CARGO_BIN_NAME"))
        .expect("couldn't get home directory");

    let cli_config_path = directories.config_dir().join("config.yaml");
    let cli_config = if cli_config_path.exists() {
        serde_yaml::from_slice(&read(cli_config_path.as_path())?)?
    } else {
        let config = CliConfig {
            index_repo_url: "https://github.com/daimond113/pesde-index".to_string(),
            cache_dir: None,
        };
        create_dir_all(directories.config_dir())?;
        config.write(&directories)?;
        config
    };

    let cwd_buf = cli
        .directory
        .or(std::env::current_dir().ok())
        .ok_or(anyhow::anyhow!("couldn't get current directory"))?;

    let api_token_entry = Entry::new(env!("CARGO_BIN_NAME"), "api_token")?;

    let mut hasher = DefaultHasher::new();
    cli_config.index_repo_url.hash(&mut hasher);
    let hash = hasher.finish().to_string();

    let index = GitIndex::new(
        cli_config.cache_dir(&directories).join("index").join(hash),
        &cli_config.index_repo_url,
        Some(Box::new(|| {
            Box::new(|a, b, c| {
                let git_authenticator = GitAuthenticator::new();
                let config = git2::Config::open_default().unwrap();
                let mut cred = git_authenticator.credentials(&config);

                cred(a, b, c)
            })
        })),
    );
    index.refresh()?;

    let mut header_map = reqwest::header::HeaderMap::new();
    header_map.insert(ACCEPT, "application/json".parse()?);
    header_map.insert("X-GitHub-Api-Version", "2022-11-28".parse()?);

    match api_token_entry.get_password() {
        Ok(api_token) => {
            header_map.insert(AUTHORIZATION, format!("Bearer {api_token}").parse()?);
        }
        Err(err) => match err {
            keyring::Error::NoEntry => {}
            _ => {
                bail!("error getting api token from keyring: {err}")
            }
        },
    };

    let reqwest_client = reqwest::blocking::Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .default_headers(header_map)
        .build()?;

    let params = CliParams {
        index,
        api_token_entry,
        reqwest_client,
        cli_config,
        cwd: cwd_buf,
        multi,
        directories,
    };

    match cli.command {
        Command::Auth { command } => auth_command(command, params),
        Command::Config { command } => config_command(command, params),
        cmd => root_command(cmd, params),
    }
}
