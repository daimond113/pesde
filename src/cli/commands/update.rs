use crate::cli::{download_graph, run_on_workspace_members};
use anyhow::Context;
use clap::Args;
use colored::Colorize;
use indicatif::MultiProgress;
use pesde::{lockfile::Lockfile, Project};
use std::{collections::HashSet, thread::JoinHandle};

#[derive(Debug, Args, Copy, Clone)]
pub struct UpdateCommand {
    /// The amount of threads to use for downloading
    #[arg(short, long, default_value_t = 6, value_parser = clap::value_parser!(u64).range(1..=128))]
    threads: u64,
}

impl UpdateCommand {
    pub fn run(
        self,
        project: Project,
        multi: MultiProgress,
        reqwest: reqwest::blocking::Client,
        update_task: &mut Option<JoinHandle<()>>,
    ) -> anyhow::Result<()> {
        let mut refreshed_sources = HashSet::new();

        let manifest = project
            .deser_manifest()
            .context("failed to read manifest")?;

        println!(
            "\n{}\n",
            format!("[now updating {} {}]", manifest.name, manifest.target)
                .bold()
                .on_bright_black()
        );

        let graph = project
            .dependency_graph(None, &mut refreshed_sources)
            .context("failed to build dependency graph")?;

        if let Some(handle) = update_task.take() {
            handle.join().expect("failed to join update task");
        }

        project
            .write_lockfile(Lockfile {
                name: manifest.name,
                version: manifest.version,
                target: manifest.target.kind(),
                overrides: manifest.overrides,

                graph: download_graph(
                    &project,
                    &mut refreshed_sources,
                    &graph,
                    &multi,
                    &reqwest,
                    self.threads as usize,
                    false,
                    false,
                    "📥 downloading dependencies".to_string(),
                    "📥 downloaded dependencies".to_string(),
                )?,

                workspace: run_on_workspace_members(&project, |project| {
                    self.run(project, multi.clone(), reqwest.clone(), &mut None)
                })?,
            })
            .context("failed to write lockfile")?;

        Ok(())
    }
}
