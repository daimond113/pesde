use std::{fs::create_dir_all, path::PathBuf};

use anyhow::Context;
use clap::Parser;
use colored::Colorize;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;

use pesde::{AuthConfig, Project, MANIFEST_FILE_NAME};

use crate::cli::{
    auth::get_token,
    config::read_config,
    home_dir,
    scripts::update_scripts_folder,
    version::{check_for_updates, current_version, get_or_download_version, max_installed_version},
    HOME_DIR,
};

mod cli;
pub mod util;

#[derive(Parser, Debug)]
#[clap(version, about = "pesde is a feature-rich package manager for Luau")]
#[command(disable_version_flag = true)]
struct Cli {
    /// Print version
    #[arg(short = 'v', short_alias = 'V', long, action = clap::builder::ArgAction::Version)]
    version: (),

    #[command(subcommand)]
    subcommand: cli::commands::Subcommand,
}

#[cfg(windows)]
fn get_root(path: &std::path::Path) -> PathBuf {
    match path.components().next().unwrap() {
        std::path::Component::Prefix(prefix) => {
            let mut string = prefix.as_os_str().to_string_lossy().to_string();
            if string.ends_with(':') {
                string.push(std::path::MAIN_SEPARATOR);
            }

            std::path::PathBuf::from(&string)
        }
        _ => unreachable!(),
    }
}

#[cfg(unix)]
fn get_root(path: &std::path::Path) -> PathBuf {
    use std::os::unix::fs::MetadataExt;

    let path = std::fs::canonicalize(path).unwrap();
    let mut current = path.as_path();

    while let Some(parent) = current.parent() {
        if std::fs::metadata(parent).unwrap().dev() != std::fs::metadata(current).unwrap().dev() {
            break;
        }

        current = parent;
    }

    current.to_path_buf()
}

fn run() -> anyhow::Result<()> {
    let cwd = std::env::current_dir().expect("failed to get current working directory");
    let project_root_dir = 'finder: {
        let mut project_root = cwd.clone();

        while project_root.components().count() > 1 {
            if project_root.join(MANIFEST_FILE_NAME).exists() {
                break 'finder project_root;
            }

            if let Some(parent) = project_root.parent() {
                project_root = parent.to_path_buf();
            } else {
                break;
            }
        }

        cwd.clone()
    };

    #[cfg(windows)]
    'scripts: {
        let exe = std::env::current_exe().expect("failed to get current executable path");
        if exe.parent().is_some_and(|parent| {
            parent.file_name().is_some_and(|parent| parent != "bin")
                || parent
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .is_some_and(|parent| parent != HOME_DIR)
        }) {
            break 'scripts;
        }

        let exe_name = exe.with_extension("");
        let exe_name = exe_name.file_name().unwrap();

        if exe_name == env!("CARGO_BIN_NAME") {
            break 'scripts;
        }

        let status = std::process::Command::new("lune")
            .arg("run")
            .arg(exe.with_extension(""))
            .args(std::env::args_os().skip(1))
            .current_dir(project_root_dir)
            .status()
            .expect("failed to run lune");

        std::process::exit(status.code().unwrap());
    }

    let multi = {
        let logger = pretty_env_logger::formatted_builder()
            .parse_env(pretty_env_logger::env_logger::Env::default().default_filter_or("info"))
            .build();
        let multi = MultiProgress::new();

        LogWrapper::new(multi.clone(), logger).try_init().unwrap();

        multi
    };

    let data_dir = home_dir()?.join("data");
    create_dir_all(&data_dir).expect("failed to create data directory");

    let token = get_token()?;

    let home_cas_dir = data_dir.join("cas");
    create_dir_all(&home_cas_dir).expect("failed to create cas directory");
    let project_root = get_root(&project_root_dir);
    let cas_dir = if get_root(&home_cas_dir) == project_root {
        home_cas_dir
    } else {
        project_root.join(HOME_DIR).join("cas")
    };

    let project = Project::new(
        project_root_dir,
        data_dir,
        cas_dir,
        AuthConfig::new()
            .with_default_token(token.clone())
            .with_token_overrides(read_config()?.token_overrides),
    );

    let reqwest = {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(token) = token {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                token.parse().context("failed to create auth header")?,
            );
        }

        headers.insert(
            reqwest::header::ACCEPT,
            "application/json"
                .parse()
                .context("failed to create accept header")?,
        );

        reqwest::blocking::Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .default_headers(headers)
            .build()?
    };

    let target_version = project
        .deser_manifest()
        .ok()
        .and_then(|manifest| manifest.pesde_version);

    // store the current version in case it needs to be used later
    get_or_download_version(&reqwest, &current_version())?;

    let exe_path = if let Some(version) = target_version {
        Some(get_or_download_version(&reqwest, &version)?)
    } else {
        None
    };
    let exe_path = if let Some(exe_path) = exe_path {
        exe_path
    } else {
        get_or_download_version(&reqwest, &max_installed_version()?)?
    };

    if let Some(exe_path) = exe_path {
        let status = std::process::Command::new(exe_path)
            .args(std::env::args_os().skip(1))
            .status()
            .expect("failed to run new version");

        std::process::exit(status.code().unwrap());
    }

    match check_for_updates(&reqwest) {
        Ok(_) => {}
        Err(e) => {
            println!(
                "{}",
                format!("failed to check for updates: {e}\n\n").red().bold()
            );
        }
    }

    match update_scripts_folder(&project) {
        Ok(_) => {}
        Err(e) => {
            println!(
                "{}",
                format!("failed to update scripts: {e}\n\n").red().bold()
            );
        }
    }

    Cli::parse().subcommand.run(project, multi, reqwest)
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}: {err}\n", "error".red().bold());

        let cause = err.chain().skip(1).collect::<Vec<_>>();

        if !cause.is_empty() {
            eprintln!("{}:", "caused by".red().bold());
            for err in cause {
                eprintln!("  - {err}");
            }
        }

        let backtrace = err.backtrace();
        match backtrace.status() {
            std::backtrace::BacktraceStatus::Disabled => {
                eprintln!(
                    "\n{}: set RUST_BACKTRACE=1 for a backtrace",
                    "help".yellow().bold()
                );
            }
            std::backtrace::BacktraceStatus::Captured => {
                eprintln!("\n{}:\n{backtrace}", "backtrace".yellow().bold());
            }
            _ => {
                eprintln!("\n{}: not captured", "backtrace".yellow().bold());
            }
        }

        std::process::exit(1);
    }
}
