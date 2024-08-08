use std::fs::remove_dir_all;

use anyhow::Context;

use pesde::Project;

use crate::cli::{config::read_config, home_dir};

pub fn update_scripts_folder(_project: &Project) -> anyhow::Result<()> {
    let scripts_dir = home_dir()?.join("scripts");

    if scripts_dir.exists() {
        // checking out the repository seems to be corrupting the repository contents
        // TODO: add actual `git pull`-esque functionality
        remove_dir_all(&scripts_dir).context("failed to remove scripts directory")?;
    }

    std::fs::create_dir_all(&scripts_dir).context("failed to create scripts directory")?;

    let cli_config = read_config()?;

    gix::prepare_clone(cli_config.scripts_repo, &scripts_dir)
        .context("failed to prepare scripts repository clone")?
        .fetch_then_checkout(gix::progress::Discard, &false.into())
        .context("failed to fetch and checkout scripts repository")?
        .0
        .main_worktree(gix::progress::Discard, &false.into())
        .context("failed to set scripts repository as main worktree")?;

    Ok(())
}
