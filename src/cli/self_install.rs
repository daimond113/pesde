use crate::cli::{home_dir, update_scripts_folder};
use anyhow::Context;
use clap::Args;
use pesde::Project;
use std::fs::create_dir_all;

#[derive(Debug, Args)]
pub struct SelfInstallCommand {}

impl SelfInstallCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        update_scripts_folder(&project)?;

        create_dir_all(home_dir()?.join("bin")).context("failed to create bin folder")?;

        // TODO: add the bin folder to the PATH

        Ok(())
    }
}
