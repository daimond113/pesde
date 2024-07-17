use anyhow::Context;
use clap::Args;
use pesde::Project;
use std::collections::HashSet;

#[derive(Debug, Args)]
pub struct InstallCommand {}

impl InstallCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        let mut refreshed_sources = HashSet::new();
        let graph = project
            .dependency_graph(None, &mut refreshed_sources)
            .context("failed to build dependency graph")?;
        let downloaded_graph = project
            .download_graph(&graph, &mut refreshed_sources)
            .context("failed to download dependencies")?;

        project
            .link_dependencies(&downloaded_graph)
            .context("failed to link dependencies")?;

        Ok(())
    }
}
