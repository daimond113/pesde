use crate::cli::IsUpToDate;
use anyhow::Context;
use clap::Args;
use pesde::{lockfile::Lockfile, Project};
use std::collections::HashSet;

#[derive(Debug, Args)]
pub struct InstallCommand {}

impl InstallCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        let mut refreshed_sources = HashSet::new();

        let manifest = project
            .deser_manifest()
            .context("failed to read manifest")?;

        let lockfile = if project
            .is_up_to_date(false)
            .context("failed to check if project is up to date")?
        {
            match project.deser_lockfile() {
                Ok(lockfile) => Some(lockfile),
                Err(pesde::errors::LockfileReadError::Io(e))
                    if e.kind() == std::io::ErrorKind::NotFound =>
                {
                    None
                }
                Err(e) => return Err(e.into()),
            }
        } else {
            None
        };

        let old_graph = lockfile.map(|lockfile| {
            lockfile
                .graph
                .into_iter()
                .map(|(name, versions)| {
                    (
                        name,
                        versions
                            .into_iter()
                            .map(|(version, node)| (version, node.node))
                            .collect(),
                    )
                })
                .collect()
        });

        let graph = project
            .dependency_graph(old_graph.as_ref(), &mut refreshed_sources)
            .context("failed to build dependency graph")?;
        let downloaded_graph = project
            .download_graph(&graph, &mut refreshed_sources)
            .context("failed to download dependencies")?;

        project
            .link_dependencies(&downloaded_graph)
            .context("failed to link dependencies")?;

        project
            .write_lockfile(Lockfile {
                name: manifest.name,
                version: manifest.version,
                overrides: manifest.overrides,

                graph: downloaded_graph,
            })
            .context("failed to write lockfile")?;

        Ok(())
    }
}
