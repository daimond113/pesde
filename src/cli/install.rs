use crate::cli::{reqwest_client, IsUpToDate};
use anyhow::Context;
use clap::Args;
use indicatif::MultiProgress;
use pesde::{lockfile::Lockfile, Project};
use std::{collections::HashSet, sync::Arc, time::Duration};

#[derive(Debug, Args)]
pub struct InstallCommand {
    /// The amount of threads to use for downloading, defaults to 6
    #[arg(short, long)]
    threads: Option<usize>,
}

impl InstallCommand {
    pub fn run(self, project: Project, multi: MultiProgress) -> anyhow::Result<()> {
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

        let bar = multi.add(
            indicatif::ProgressBar::new(graph.values().map(|versions| versions.len() as u64).sum())
                .with_style(
                    indicatif::ProgressStyle::default_bar().template(
                        "{msg} {bar:40.208/166} {pos}/{len} {percent}% {elapsed_precise}",
                    )?,
                )
                .with_message("downloading dependencies"),
        );
        bar.enable_steady_tick(Duration::from_millis(100));

        let (rx, downloaded_graph) = project
            .download_graph(
                &graph,
                &mut refreshed_sources,
                &reqwest_client(project.data_dir())?,
                self.threads.unwrap_or(6).max(1),
            )
            .context("failed to download dependencies")?;

        while let Ok(result) = rx.recv() {
            bar.inc(1);

            match result {
                Ok(()) => {}
                Err(e) => return Err(e.into()),
            }
        }

        bar.finish_with_message("finished downloading dependencies");

        let downloaded_graph = Arc::into_inner(downloaded_graph)
            .unwrap()
            .into_inner()
            .unwrap();

        project
            .link_dependencies(&downloaded_graph)
            .context("failed to link dependencies")?;

        project
            .write_lockfile(Lockfile {
                name: manifest.name,
                version: manifest.version,
                target: manifest.target.kind(),
                overrides: manifest.overrides,

                graph: downloaded_graph,
            })
            .context("failed to write lockfile")?;

        Ok(())
    }
}
