use crate::cli::{files::make_executable, home_dir, scripts::update_scripts_folder, HOME_DIR};
use anyhow::Context;
use clap::Args;
use colored::Colorize;
use pesde::Project;
use std::fs::create_dir_all;

#[derive(Debug, Args)]
pub struct SelfInstallCommand {
    #[cfg(windows)]
    #[arg(short, long)]
    skip_add_to_path: bool,
}

impl SelfInstallCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        update_scripts_folder(&project)?;

        let bin_dir = home_dir()?.join("bin");
        create_dir_all(&bin_dir).context("failed to create bin folder")?;

        #[cfg(windows)]
        if !self.skip_add_to_path {
            use winreg::{enums::HKEY_CURRENT_USER, RegKey};

            let current_user = RegKey::predef(HKEY_CURRENT_USER);
            let env = current_user
                .create_subkey("Environment")
                .context("failed to open Environment key")?
                .0;
            let path: String = env.get_value("Path").context("failed to get Path value")?;

            let exists = path
                .split(';')
                .any(|part| part == bin_dir.to_string_lossy().as_ref());

            if !exists {
                let new_path = format!("{path};{}", bin_dir.to_string_lossy());
                env.set_value("Path", &new_path)
                    .context("failed to set Path value")?;
            }

            println!(
                "installed {} {}!",
                env!("CARGO_PKG_NAME").cyan(),
                env!("CARGO_PKG_VERSION").yellow(),
            );

            if !exists {
                println!(
                    "\nin order to allow binary exports as executables {}.\n\n{}",
                    format!("`~/{HOME_DIR}/bin` was added to PATH").green(),
                    "please restart your shell for this to take effect"
                        .yellow()
                        .bold()
                );
            }
        }

        #[cfg(unix)]
        {
            println!(
                r#"installed {} {}! in order to be able to run binary exports as programs, add the following line to your shell profile:

{}

and then restart your shell.
"#,
                env!("CARGO_PKG_NAME").cyan(),
                env!("CARGO_PKG_VERSION").yellow(),
                format!(r#"export PATH="$PATH:~/{}/bin""#, HOME_DIR)
                    .bold()
                    .green()
            );
        }

        let copy_to = bin_dir
            .join(env!("CARGO_BIN_NAME"))
            .with_extension(std::env::consts::EXE_EXTENSION);

        std::fs::copy(std::env::current_exe()?, &copy_to)
            .context("failed to copy executable to bin folder")?;

        make_executable(&copy_to)?;

        Ok(())
    }
}
