use crate::cli::IsUpToDate;
use anyhow::Context;
use clap::Args;
use pesde::{
    names::{PackageName, PackageNames},
    scripts::execute_script,
    Project, PACKAGES_CONTAINER_NAME,
};
use relative_path::RelativePathBuf;

#[derive(Debug, Args)]
pub struct RunCommand {
    /// The package name, script name, or path to a script to run
    #[arg(index = 1)]
    package_or_script: String,

    /// Arguments to pass to the script
    #[arg(index = 2, last = true)]
    args: Vec<String>,
}

impl RunCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        if let Ok(pkg_name) = self.package_or_script.parse::<PackageName>() {
            let graph = if project.is_up_to_date(true)? {
                project.deser_lockfile()?.graph
            } else {
                anyhow::bail!("outdated lockfile, please run the install command first")
            };

            let pkg_name = PackageNames::Pesde(pkg_name);

            for (version, node) in graph.get(&pkg_name).context("package not found in graph")? {
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
                    version,
                );

                let path = bin_path.to_path(&container_folder);

                execute_script(
                    Some(pkg_name.as_str().1),
                    &path,
                    &self.args,
                    project.path(),
                    false,
                )
                .context("failed to execute script")?;
            }
        }

        if let Ok(manifest) = project.deser_manifest() {
            if let Some(script_path) = manifest.scripts.get(&self.package_or_script) {
                execute_script(
                    Some(&self.package_or_script),
                    &script_path.to_path(project.path()),
                    &self.args,
                    project.path(),
                    false,
                )
                .context("failed to execute script")?;

                return Ok(());
            }
        };

        let relative_path = RelativePathBuf::from(self.package_or_script);
        let path = relative_path.to_path(project.path());

        if !path.exists() {
            anyhow::bail!("path does not exist: {}", path.display());
        }

        execute_script(None, &path, &self.args, project.path(), false)
            .context("failed to execute script")?;

        Ok(())
    }
}
