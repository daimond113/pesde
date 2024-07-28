use crate::cli::{
    config::read_config,
    version::{get_or_download_version, update_bin_exe},
};
use clap::Args;

#[derive(Debug, Args)]
pub struct SelfUpgradeCommand {}

impl SelfUpgradeCommand {
    pub fn run(self, reqwest: reqwest::blocking::Client) -> anyhow::Result<()> {
        let config = read_config()?;

        get_or_download_version(&reqwest, &config.last_checked_updates.unwrap().1)?;
        update_bin_exe()?;

        Ok(())
    }
}
