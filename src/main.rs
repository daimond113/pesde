use crate::cli::get_token;
use clap::Parser;
use colored::Colorize;
use pesde::{AuthConfig, Project};
use std::fs::create_dir_all;

mod cli;

#[derive(Parser, Debug)]
#[clap(version, about = "pesde is a feature-rich package manager for Luau")]
#[command(disable_version_flag = true)]
struct Cli {
    /// Print version
    #[arg(short = 'v', short_alias = 'V', long, action = clap::builder::ArgAction::Version)]
    version: (),

    #[command(subcommand)]
    subcommand: cli::Subcommand,
}

fn main() {
    pretty_env_logger::init();

    let project_dirs =
        directories::ProjectDirs::from("com", env!("CARGO_PKG_NAME"), env!("CARGO_BIN_NAME"))
            .expect("couldn't get home directory");
    let cwd = std::env::current_dir().expect("failed to get current working directory");
    let cli = Cli::parse();

    let data_dir = project_dirs.data_dir();
    create_dir_all(data_dir).expect("failed to create data directory");

    if let Err(err) = get_token(data_dir).and_then(|token| {
        cli.subcommand.run(Project::new(
            cwd,
            data_dir,
            AuthConfig::new().with_pesde_token(token),
        ))
    }) {
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
