use std::{ffi::OsString, path::PathBuf, process::Command};

use anyhow::Context;
use clap::Args;
use relative_path::RelativePathBuf;

use pesde::{
    names::{PackageName, PackageNames},
    Project, PACKAGES_CONTAINER_NAME,
};

use crate::cli::IsUpToDate;

#[derive(Debug, Args)]
pub struct RunCommand {
    /// The package name, script name, or path to a script to run
    #[arg(index = 1)]
    package_or_script: String,

    /// Arguments to pass to the script
    #[arg(index = 2, last = true)]
    args: Vec<OsString>,
}

impl RunCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        let run = |path: PathBuf| {
            let status = Command::new("lune")
                .arg("run")
                .arg(path)
                .arg("--")
                .args(&self.args)
                .current_dir(project.path())
                .status()
                .expect("failed to run script");

            std::process::exit(status.code().unwrap_or(1))
        };

        if let Ok(pkg_name) = self.package_or_script.parse::<PackageName>() {
            let graph = if project.is_up_to_date(true)? {
                project.deser_lockfile()?.graph
            } else {
                anyhow::bail!("outdated lockfile, please run the install command first")
            };

            let pkg_name = PackageNames::Pesde(pkg_name);

            for (version_id, node) in graph.get(&pkg_name).context("package not found in graph")? {
                if node.node.direct.is_none() {
                    continue;
                }

                let Some(bin_path) = node.target.bin_path() else {
                    anyhow::bail!("package has no bin path");
                };

                let base_folder = node
                    .node
                    .base_folder(project.deser_manifest()?.target.kind(), true);
                let container_folder = node.node.container_folder(
                    &project
                        .path()
                        .join(base_folder)
                        .join(PACKAGES_CONTAINER_NAME),
                    &pkg_name,
                    version_id.version(),
                );

                run(bin_path.to_path(&container_folder))
            }
        }

        if let Ok(manifest) = project.deser_manifest() {
            if let Some(script_path) = manifest.scripts.get(&self.package_or_script) {
                run(script_path.to_path(project.path()))
            }
        };

        let relative_path = RelativePathBuf::from(self.package_or_script);
        let path = relative_path.to_path(project.path());

        if !path.exists() {
            anyhow::bail!("path does not exist: {}", path.display());
        }

        run(path);

        Ok(())
    }
}
