use crate::cli::{bin_dir, files::make_executable};
use anyhow::Context;
use clap::Args;
use colored::{ColoredString, Colorize};
use indicatif::MultiProgress;
use pesde::{lockfile::Lockfile, manifest::target::TargetKind, Project, MANIFEST_FILE_NAME};
use relative_path::RelativePathBuf;
use std::{
    collections::{BTreeSet, HashSet},
    sync::Arc,
    time::Duration,
};

#[derive(Debug, Args)]
pub struct InstallCommand {
    /// The amount of threads to use for downloading
    #[arg(short, long, default_value_t = 6, value_parser = clap::value_parser!(u64).range(1..=128))]
    threads: u64,

    /// Whether to ignore the lockfile, refreshing it
    #[arg(short, long)]
    pub unlocked: bool,
}

fn bin_link_file(alias: &str) -> String {
    let mut all_combinations = BTreeSet::new();

    for a in TargetKind::VARIANTS {
        for b in TargetKind::VARIANTS {
            all_combinations.insert((a, b));
        }
    }

    let all_folders = all_combinations
        .into_iter()
        .map(|(a, b)| format!("{:?}", a.packages_folder(b)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");

    #[cfg(not(unix))]
    let prefix = String::new();
    #[cfg(unix)]
    let prefix = "#!/usr/bin/env -S lune run\n";
    // TODO: reimplement workspace support in this
    format!(
        r#"{prefix}local process = require("@lune/process")
local fs = require("@lune/fs")

local project_root = process.cwd
local path_components = string.split(string.gsub(project_root, "\\", "/"), "/")

for i = #path_components, 1, -1 do
    local path = table.concat(path_components, "/", 1, i)
    if fs.isFile(path .. "/{MANIFEST_FILE_NAME}") then
        project_root = path
        break
    end
end

for _, packages_folder in {{ {all_folders} }} do
    local path = `{{project_root}}/{{packages_folder}}/{alias}.bin.luau`
    
    if fs.isFile(path) then
        require(path)
        break
    end
end
    "#,
    )
}

#[cfg(feature = "patches")]
const JOBS: u8 = 6;
#[cfg(not(feature = "patches"))]
const JOBS: u8 = 5;

fn job(n: u8) -> ColoredString {
    format!("[{n}/{JOBS}]").dimmed().bold()
}

impl InstallCommand {
    pub fn run(
        self,
        project: Project,
        multi: MultiProgress,
        reqwest: reqwest::blocking::Client,
    ) -> anyhow::Result<()> {
        let mut refreshed_sources = HashSet::new();

        let manifest = project
            .deser_manifest()
            .context("failed to read manifest")?;

        let lockfile = if self.unlocked {
            None
        } else {
            match project.deser_lockfile() {
                Ok(lockfile) => {
                    if lockfile.overrides != manifest.overrides {
                        log::debug!("overrides are different");
                        None
                    } else if lockfile.target != manifest.target.kind() {
                        log::debug!("target kind is different");
                        None
                    } else {
                        Some(lockfile)
                    }
                }
                Err(pesde::errors::LockfileReadError::Io(e))
                    if e.kind() == std::io::ErrorKind::NotFound =>
                {
                    None
                }
                Err(e) => return Err(e.into()),
            }
        };

        println!(
            "\n{}\n",
            format!("[now installing {}]", manifest.name)
                .bold()
                .on_bright_black()
        );

        println!("{} ❌ removing current package folders", job(1));

        {
            let mut deleted_folders = HashSet::new();

            for target_kind in TargetKind::VARIANTS {
                let folder = manifest.target.kind().packages_folder(target_kind);

                if deleted_folders.insert(folder.to_string()) {
                    log::debug!("deleting the {folder} folder");

                    if let Some(e) = std::fs::remove_dir_all(project.package_dir().join(&folder))
                        .err()
                        .filter(|e| e.kind() != std::io::ErrorKind::NotFound)
                    {
                        return Err(e).context(format!("failed to remove the {folder} folder"));
                    };
                }
            }
        }

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

        println!("{} 📦 building dependency graph", job(2));

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
                .with_message(format!("{} 📥 downloading dependencies", job(3))),
        );
        bar.enable_steady_tick(Duration::from_millis(100));

        let (rx, downloaded_graph) = project
            .download_graph(
                &graph,
                &mut refreshed_sources,
                &reqwest,
                self.threads as usize,
            )
            .context("failed to download dependencies")?;

        while let Ok(result) = rx.recv() {
            bar.inc(1);

            match result {
                Ok(()) => {}
                Err(e) => return Err(e.into()),
            }
        }

        bar.finish_with_message(format!("{} 📥 downloaded dependencies", job(3),));

        let downloaded_graph = Arc::into_inner(downloaded_graph)
            .unwrap()
            .into_inner()
            .unwrap();

        println!("{} 🗺️ linking dependencies", job(4));

        project
            .link_dependencies(&downloaded_graph)
            .context("failed to link dependencies")?;

        let bin_folder = bin_dir()?;

        for versions in downloaded_graph.values() {
            for node in versions.values() {
                if node.target.bin_path().is_none() {
                    continue;
                }

                let Some((alias, _)) = &node.node.direct else {
                    continue;
                };

                if alias == env!("CARGO_BIN_NAME") {
                    log::warn!("package {alias} has the same name as the CLI, skipping bin link");
                    continue;
                }

                let bin_file = bin_folder.join(alias);
                std::fs::write(&bin_file, bin_link_file(alias))
                    .context("failed to write bin link file")?;

                make_executable(&bin_file).context("failed to make bin link executable")?;

                #[cfg(windows)]
                {
                    let bin_file = bin_file.with_extension(std::env::consts::EXE_EXTENSION);
                    std::fs::copy(
                        std::env::current_exe().context("failed to get current executable path")?,
                        &bin_file,
                    )
                    .context("failed to copy bin link file")?;
                }
            }
        }

        #[cfg(feature = "patches")]
        {
            println!("{} 🩹 applying patches", job(5));

            project
                .apply_patches(&downloaded_graph)
                .context("failed to apply patches")?;
        }

        println!("{} 🧹 finishing up", job(JOBS));

        project
            .write_lockfile(Lockfile {
                name: manifest.name,
                version: manifest.version,
                target: manifest.target.kind(),
                overrides: manifest.overrides,
                workspace: match project.workspace_dir() {
                    Some(_) => {
                        // this might seem counterintuitive, but remember that the workspace
                        // is the package_dir when the user isn't in a member package
                        Default::default()
                    }
                    None => project
                        .workspace_members(project.package_dir())
                        .context("failed to get workspace members")?
                        .into_iter()
                        .map(|(path, manifest)| {
                            (
                                manifest.name,
                                RelativePathBuf::from_path(
                                    path.strip_prefix(project.package_dir()).unwrap(),
                                )
                                .unwrap(),
                            )
                        })
                        .map(|(name, path)| {
                            InstallCommand {
                                threads: self.threads,
                                unlocked: self.unlocked,
                            }
                            .run(
                                Project::new(
                                    path.to_path(project.package_dir()),
                                    Some(project.package_dir()),
                                    project.data_dir(),
                                    project.cas_dir(),
                                    project.auth_config().clone(),
                                ),
                                multi.clone(),
                                reqwest.clone(),
                            )
                            .map(|_| (name, path))
                        })
                        .collect::<Result<_, _>>()
                        .context("failed to install workspace member's dependencies")?,
                },

                graph: downloaded_graph,
            })
            .context("failed to write lockfile")?;

        Ok(())
    }
}
