use std::{ffi::OsString, process::Command};

use anyhow::Context;
use clap::Args;
use semver::VersionReq;

use crate::cli::{config::read_config, VersionedPackageName};
use pesde::{
    manifest::target::TargetKind,
    names::PackageName,
    source::{
        pesde::{specifier::PesdeDependencySpecifier, PesdePackageSource},
        traits::PackageSource,
    },
    Project,
};

#[derive(Debug, Args)]
pub struct ExecuteCommand {
    /// The package name, script name, or path to a script to run
    #[arg(index = 1)]
    package: VersionedPackageName<VersionReq, PackageName>,

    /// The index URL to use for the package
    #[arg(short, long, value_parser = crate::cli::parse_gix_url)]
    index: Option<gix::Url>,

    /// Arguments to pass to the script
    #[arg(index = 2, last = true)]
    args: Vec<OsString>,
}

impl ExecuteCommand {
    pub fn run(self, project: Project, reqwest: reqwest::blocking::Client) -> anyhow::Result<()> {
        let index = self
            .index
            .or_else(|| read_config().ok().map(|c| c.default_index))
            .context("no index specified")?;
        let source = PesdePackageSource::new(index);
        source
            .refresh(&project)
            .context("failed to refresh source")?;

        let mut results = source
            .resolve(
                &PesdeDependencySpecifier {
                    name: self.package.0,
                    version: self.package.1.unwrap_or(VersionReq::STAR),
                    index: None,
                    target: None,
                },
                &project,
                TargetKind::Lune,
            )
            .context("failed to resolve package")?;

        let (version, pkg_ref) = results.1.pop_last().context("no package found")?;

        log::info!("found package {}@{version}", pkg_ref.name);

        let (fs, target) = source
            .download(&pkg_ref, &project, &reqwest)
            .context("failed to download package")?;
        let bin_path = target.bin_path().context("package has no binary export")?;

        let tmp_dir = project.cas_dir().join(".tmp");
        std::fs::create_dir_all(&tmp_dir).context("failed to create temporary directory")?;

        let tempdir =
            tempfile::tempdir_in(tmp_dir).context("failed to create temporary directory")?;

        fs.write_to(tempdir.path(), project.cas_dir(), true)
            .context("failed to write package contents")?;

        let status = Command::new("lune")
            .arg("run")
            .arg(bin_path.to_path(tempdir.path()))
            .arg("--")
            .args(&self.args)
            .current_dir(project.path())
            .status()
            .context("failed to run script")?;

        drop(tempdir);

        std::process::exit(status.code().unwrap_or(1))
    }
}
