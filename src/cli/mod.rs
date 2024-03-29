use crate::cli::{api_token::API_TOKEN_SOURCE, auth::AuthCommand, config::ConfigCommand};
use auth_git2::GitAuthenticator;
use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::error;
use once_cell::sync::Lazy;
use pesde::{
    index::{GitIndex, Index},
    manifest::{Manifest, Realm},
    package_name::{PackageName, StandardPackageName},
    project::DEFAULT_INDEX_NAME,
};
use pretty_env_logger::env_logger::Env;
use reqwest::{
    blocking::{RequestBuilder, Response},
    header::ACCEPT,
};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::{
    fs::create_dir_all,
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    str::FromStr,
};

pub mod api_token;
pub mod auth;
pub mod config;
pub mod root;

#[derive(Debug, Clone)]
pub struct VersionedPackageName<V: FromStr<Err = semver::Error>>(PackageName, V);

impl<V: FromStr<Err = semver::Error>> FromStr for VersionedPackageName<V> {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, version) = s.split_once('@').ok_or_else(|| {
            anyhow::anyhow!("invalid package name: {s}; expected format: name@version")
        })?;

        Ok(VersionedPackageName(
            name.to_string().parse()?,
            version.parse()?,
        ))
    }
}

#[derive(Subcommand, Clone)]
pub enum Command {
    /// Initializes a manifest file
    Init,

    /// Adds a package to the manifest
    Add {
        /// The package to add
        #[clap(value_name = "PACKAGE")]
        package: VersionedPackageName<VersionReq>,

        /// Whether the package is a peer dependency
        #[clap(long, short)]
        peer: bool,

        /// The realm of the package
        #[clap(long, short)]
        realm: Option<Realm>,
    },

    /// Removes a package from the manifest
    Remove {
        /// The package to remove
        #[clap(value_name = "PACKAGE")]
        package: PackageName,
    },

    /// Lists outdated packages
    Outdated,

    /// Installs the dependencies of the project
    Install {
        /// Whether to use the lockfile for resolving dependencies
        #[clap(long, short)]
        locked: bool,
    },

    /// Runs the `bin` export of the specified package
    Run {
        /// The package to run
        #[clap(value_name = "PACKAGE")]
        package: StandardPackageName,

        /// The arguments to pass to the package
        #[clap(last = true)]
        args: Vec<String>,
    },

    /// Searches for a package on the registry
    Search {
        /// The query to search for
        #[clap(value_name = "QUERY")]
        query: Option<String>,
    },

    /// Publishes the project to the registry
    Publish,

    /// Converts a `wally.toml` file to a `pesde.yaml` file
    #[cfg(feature = "wally")]
    Convert,

    /// Begins a new patch
    Patch {
        /// The package to patch
        #[clap(value_name = "PACKAGE")]
        package: VersionedPackageName<Version>,
    },

    /// Commits (finishes) the patch
    PatchCommit {
        /// The package's changed directory
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

#[derive(Parser, Clone)]
#[clap(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,

    /// The directory to run the command in
    #[arg(short, long, value_name = "DIRECTORY")]
    pub directory: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CliConfig {
    pub cache_dir: Option<PathBuf>,
}

impl CliConfig {
    pub fn cache_dir(&self) -> PathBuf {
        self.cache_dir
            .clone()
            .unwrap_or_else(|| DIRS.cache_dir().to_path_buf())
    }

    pub fn open() -> anyhow::Result<Self> {
        let cli_config_path = DIRS.config_dir().join("config.yaml");

        if cli_config_path.exists() {
            Ok(serde_yaml::from_slice(&std::fs::read(cli_config_path)?)?)
        } else {
            let config = CliConfig::default();
            config.write()?;
            Ok(config)
        }
    }

    pub fn write(&self) -> anyhow::Result<()> {
        let folder = DIRS.config_dir();
        create_dir_all(folder)?;
        serde_yaml::to_writer(
            &mut std::fs::File::create(folder.join("config.yaml"))?,
            &self,
        )?;

        Ok(())
    }
}

pub fn send_request(request_builder: RequestBuilder) -> anyhow::Result<Response> {
    let res = request_builder.send()?;

    match res.error_for_status_ref() {
        Ok(_) => Ok(res),
        Err(e) => {
            error!("request failed: {e}\nbody: {}", res.text()?);
            Err(e.into())
        }
    }
}

pub static CLI: Lazy<Cli> = Lazy::new(Cli::parse);

pub static DIRS: Lazy<ProjectDirs> = Lazy::new(|| {
    ProjectDirs::from("com", env!("CARGO_PKG_NAME"), env!("CARGO_BIN_NAME"))
        .expect("couldn't get home directory")
});

pub static CLI_CONFIG: Lazy<CliConfig> = Lazy::new(|| CliConfig::open().unwrap());

pub static CWD: Lazy<PathBuf> = Lazy::new(|| {
    CLI.directory
        .clone()
        .or(std::env::current_dir().ok())
        .expect("couldn't get current directory")
});

pub static REQWEST_CLIENT: Lazy<reqwest::blocking::Client> = Lazy::new(|| {
    let mut header_map = reqwest::header::HeaderMap::new();
    header_map.insert(ACCEPT, "application/json".parse().unwrap());
    header_map.insert("X-GitHub-Api-Version", "2022-11-28".parse().unwrap());

    if let Ok(Some(token)) = API_TOKEN_SOURCE.get_api_token() {
        header_map.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );
    }

    reqwest::blocking::Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .default_headers(header_map)
        .build()
        .unwrap()
});

pub static MULTI: Lazy<MultiProgress> = Lazy::new(|| {
    let logger = pretty_env_logger::formatted_builder()
        .parse_env(Env::default().default_filter_or("info"))
        .build();
    let multi = MultiProgress::new();

    LogWrapper::new(multi.clone(), logger).try_init().unwrap();

    multi
});

pub const DEFAULT_INDEX_URL: &str = "https://github.com/daimond113/pesde-index";
#[cfg(feature = "wally")]
pub const DEFAULT_WALLY_INDEX_URL: &str = "https://github.com/UpliftGames/wally-index";

pub fn index_dir(url: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = hasher.finish().to_string();

    CLI_CONFIG
        .cache_dir()
        .join("indices")
        .join(hash)
        .join("index")
}

pub fn clone_index(url: &str) -> GitIndex {
    let index = GitIndex::new(
        index_dir(url),
        &url.parse().unwrap(),
        Some(Box::new(|| {
            Box::new(|a, b, c| {
                let git_authenticator = GitAuthenticator::new();
                let config = git2::Config::open_default().unwrap();
                let mut cred = git_authenticator.credentials(&config);

                cred(a, b, c)
            })
        })),
        API_TOKEN_SOURCE.get_api_token().unwrap(),
    );

    index.refresh().unwrap();

    index
}

pub static DEFAULT_INDEX_DATA: Lazy<(PathBuf, String)> = Lazy::new(|| {
    let manifest = Manifest::from_path(CWD.to_path_buf())
        .map(|m| m.indices.get(DEFAULT_INDEX_NAME).unwrap().clone());
    let url = &manifest.unwrap_or(DEFAULT_INDEX_URL.to_string());

    (index_dir(url), url.clone())
});

pub static DEFAULT_INDEX: Lazy<GitIndex> = Lazy::new(|| clone_index(&DEFAULT_INDEX_DATA.1));
