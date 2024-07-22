use crate::util::authenticate_conn;
use anyhow::Context;
use gix::remote::Direction;
use keyring::Entry;
use pesde::Project;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

mod auth;
mod config;
mod init;
mod install;
mod publish;
mod run;
mod self_install;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub default_index: url::Url,
    pub scripts_repo: url::Url,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            default_index: "https://github.com/daimond113/pesde-index".parse().unwrap(),
            scripts_repo: "https://github.com/daimond113/pesde-scripts"
                .parse()
                .unwrap(),
            token: None,
        }
    }
}

pub fn read_config(data_dir: &Path) -> anyhow::Result<CliConfig> {
    let config_string = match std::fs::read_to_string(data_dir.join("config.toml")) {
        Ok(config_string) => config_string,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(CliConfig::default());
        }
        Err(e) => return Err(e).context("failed to read config file"),
    };

    let config = toml::from_str(&config_string).context("failed to parse config file")?;

    Ok(config)
}

pub fn write_config(data_dir: &Path, config: &CliConfig) -> anyhow::Result<()> {
    let config_string = toml::to_string(config).context("failed to serialize config")?;
    std::fs::write(data_dir.join("config.toml"), config_string)
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
            format!("Bearer {token}")
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

pub fn update_scripts_folder(project: &Project) -> anyhow::Result<()> {
    let home_dir = directories::UserDirs::new()
        .context("failed to get home directory")?
        .home_dir()
        .to_owned();

    let scripts_dir = home_dir
        .join(concat!(".", env!("CARGO_PKG_NAME")))
        .join("scripts");

    if scripts_dir.exists() {
        let repo = gix::open(&scripts_dir).context("failed to open scripts repository")?;

        let remote = repo
            .find_default_remote(Direction::Fetch)
            .context("missing default remote of scripts repository")?
            .context("failed to find default remote of scripts repository")?;

        let mut connection = remote
            .connect(Direction::Fetch)
            .context("failed to connect to default remote of scripts repository")?;

        authenticate_conn(&mut connection, project.auth_config());

        let results = connection
            .prepare_fetch(gix::progress::Discard, Default::default())
            .context("failed to prepare scripts repository fetch")?
            .receive(gix::progress::Discard, &false.into())
            .context("failed to receive new scripts repository contents")?;

        let remote_ref = results
            .ref_map
            .remote_refs
            .first()
            .context("failed to get remote refs of scripts repository")?;

        let unpacked = remote_ref.unpack();
        let oid = unpacked
            .1
            .or(unpacked.2)
            .context("couldn't find oid in remote ref")?;

        let tree = repo
            .find_object(oid)
            .context("failed to find scripts repository tree")?
            .peel_to_tree()
            .context("failed to peel scripts repository object to tree")?;

        let mut index = gix::index::File::from_state(
            gix::index::State::from_tree(&tree.id, &repo.objects, Default::default())
                .context("failed to create index state from scripts repository tree")?,
            repo.index_path(),
        );

        let opts = gix::worktree::state::checkout::Options {
            overwrite_existing: true,
            destination_is_initially_empty: false,
            ..Default::default()
        };

        gix::worktree::state::checkout(
            &mut index,
            repo.work_dir().context("scripts repo is bare")?,
            repo.objects
                .clone()
                .into_arc()
                .context("failed to clone objects")?,
            &gix::progress::Discard,
            &gix::progress::Discard,
            &false.into(),
            opts,
        )
        .context("failed to checkout scripts repository")?;

        index
            .write(gix::index::write::Options::default())
            .context("failed to write index")?;
    } else {
        std::fs::create_dir_all(&scripts_dir).context("failed to create scripts directory")?;

        let cli_config = read_config(project.data_dir())?;

        gix::prepare_clone(cli_config.scripts_repo.as_str(), &scripts_dir)
            .context("failed to prepare scripts repository clone")?
            .fetch_then_checkout(gix::progress::Discard, &false.into())
            .context("failed to fetch and checkout scripts repository")?
            .0
            .main_worktree(gix::progress::Discard, &false.into())
            .context("failed to set scripts repository as main worktree")?;
    };

    Ok(())
}

pub trait IsUpToDate {
    fn is_up_to_date(&self, strict: bool) -> anyhow::Result<bool>;
}

impl IsUpToDate for Project {
    fn is_up_to_date(&self, strict: bool) -> anyhow::Result<bool> {
        let manifest = self.deser_manifest()?;
        let lockfile = match self.deser_lockfile() {
            Ok(lockfile) => lockfile,
            Err(pesde::errors::LockfileReadError::Io(e))
                if e.kind() == std::io::ErrorKind::NotFound =>
            {
                return Ok(false);
            }
            Err(e) => return Err(e.into()),
        };

        if !strict {
            // the resolver will use the old lockfile and update it itself. it can't handle overrides only
            return Ok(manifest.overrides == lockfile.overrides);
        }

        if manifest.name != lockfile.name || manifest.version != lockfile.version {
            return Ok(false);
        }

        let specs = lockfile
            .graph
            .into_iter()
            .flat_map(|(_, versions)| versions)
            .filter_map(|(_, node)| match node.node.direct {
                Some((_, spec)) => Some((spec, node.node.ty)),
                None => None,
            })
            .collect::<HashSet<_>>();

        Ok(manifest
            .all_dependencies()
            .context("failed to get all dependencies")?
            .iter()
            .all(|(_, (spec, ty))| specs.contains(&(spec.clone(), *ty))))
    }
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

    /// Publishes the project to the registry
    Publish(publish::PublishCommand),

    /// Installs the pesde binary and scripts
    SelfInstall(self_install::SelfInstallCommand),
}

impl Subcommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        match self {
            Subcommand::Auth(auth) => auth.run(project),
            Subcommand::Config(config) => config.run(project),
            Subcommand::Init(init) => init.run(project),
            Subcommand::Run(run) => run.run(project),
            Subcommand::Install(install) => install.run(project),
            Subcommand::Publish(publish) => publish.run(project),
            Subcommand::SelfInstall(self_install) => self_install.run(project),
        }
    }
}
