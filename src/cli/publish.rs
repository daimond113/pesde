use anyhow::Context;
use clap::Args;
use colored::Colorize;
use pesde::{manifest::target::Target, Project, MANIFEST_FILE_NAME, MAX_ARCHIVE_SIZE};
use std::path::Component;

#[derive(Debug, Args)]
pub struct PublishCommand {
    /// Whether to output a tarball instead of publishing
    #[arg(short, long)]
    dry_run: bool,
}

impl PublishCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        let mut manifest = project
            .deser_manifest()
            .context("failed to read manifest")?;

        if manifest.private {
            println!("{}", "package is private, cannot publish".red().bold());

            return Ok(());
        }

        manifest
            .target
            .validate_publish()
            .context("manifest not fit for publishing")?;

        let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
            vec![],
            flate2::Compression::best(),
        ));

        let mut display_includes: Vec<String> = vec![MANIFEST_FILE_NAME.to_string()];
        #[cfg(feature = "roblox")]
        let mut display_build_files: Vec<String> = vec![];

        let (lib_path, bin_path) = (
            manifest.target.lib_path().cloned(),
            manifest.target.bin_path().cloned(),
        );

        #[cfg(feature = "roblox")]
        let mut roblox_target = match &mut manifest.target {
            Target::Roblox { build_files, .. } => Some(build_files),
            _ => None,
        };
        #[cfg(not(feature = "roblox"))]
        let roblox_target = None::<()>;

        if !manifest.includes.insert(MANIFEST_FILE_NAME.to_string()) {
            display_includes.push(MANIFEST_FILE_NAME.to_string());

            println!(
                "{}: {MANIFEST_FILE_NAME} was not in includes, adding it",
                "warn".yellow().bold()
            );
        }

        if manifest.includes.remove(".git") {
            println!(
                "{}: .git was in includes, removing it",
                "warn".yellow().bold()
            );
        }

        for (name, path) in [("lib path", lib_path), ("bin path", bin_path)] {
            let Some(export_path) = path else { continue };

            let export_path = export_path.to_path(project.path());
            if !export_path.exists() {
                anyhow::bail!("{name} points to non-existent file");
            }

            if !export_path.is_file() {
                anyhow::bail!("{name} must point to a file");
            }

            let contents =
                std::fs::read_to_string(&export_path).context(format!("failed to read {name}"))?;

            if let Err(err) = full_moon::parse(&contents).map_err(|errs| {
                errs.into_iter()
                    .map(|err| err.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            }) {
                anyhow::bail!("{name} is not a valid Luau file: {err}");
            }

            let first_part = export_path
                .strip_prefix(project.path())
                .context(format!("{name} not within project directory"))?
                .components()
                .next()
                .context(format!("{name} must contain at least one part"))?;

            let first_part = match first_part {
                Component::Normal(part) => part,
                _ => anyhow::bail!("{name} must be within project directory"),
            };

            let first_part_str = first_part.to_string_lossy();

            if manifest.includes.insert(first_part_str.to_string()) {
                println!(
                    "{}: {name} was not in includes, adding {first_part_str}",
                    "warn".yellow().bold()
                );
            }

            if roblox_target.as_mut().map_or(false, |build_files| {
                build_files.insert(first_part_str.to_string())
            }) {
                println!(
                    "{}: {name} was not in build files, adding {first_part_str}",
                    "warn".yellow().bold()
                );
            }
        }

        for included_name in &manifest.includes {
            let included_path = project.path().join(included_name);

            if !included_path.exists() {
                anyhow::bail!("included file {included_name} does not exist");
            }

            // it'll be included later, with our mut modifications
            if included_name.eq_ignore_ascii_case(MANIFEST_FILE_NAME) {
                continue;
            }

            if included_path.is_file() {
                display_includes.push(included_name.clone());

                archive.append_file(
                    included_name,
                    &mut std::fs::File::open(&included_path)
                        .context(format!("failed to read {included_name}"))?,
                )?;
            } else {
                display_includes.push(format!("{included_name}/*"));

                archive
                    .append_dir_all(included_name, &included_path)
                    .context(format!("failed to include directory {included_name}"))?;
            }
        }

        if let Some(build_files) = &roblox_target {
            for build_file in build_files.iter() {
                if build_file.eq_ignore_ascii_case(MANIFEST_FILE_NAME) {
                    println!(
                        "{}: {MANIFEST_FILE_NAME} is in build files, please remove it",
                        "warn".yellow().bold()
                    );

                    continue;
                }

                let build_file_path = project.path().join(build_file);

                if !build_file_path.exists() {
                    anyhow::bail!("build file {build_file} does not exist");
                }

                if !manifest.includes.contains(build_file) {
                    anyhow::bail!("build file {build_file} is not in includes, please add it");
                }

                if build_file_path.is_file() {
                    display_build_files.push(build_file.clone());
                } else {
                    display_build_files.push(format!("{build_file}/*"));
                }
            }
        }

        {
            println!("\n{}", "please confirm the following information:".bold());
            println!("name: {}", manifest.name);
            println!("version: {}", manifest.version);
            println!(
                "description: {}",
                manifest.description.as_deref().unwrap_or("(none)")
            );
            println!(
                "license: {}",
                manifest.license.as_deref().unwrap_or("(none)")
            );
            println!(
                "authors: {}",
                manifest
                    .authors
                    .as_ref()
                    .map_or("(none)".to_string(), |a| a.join(", "))
            );
            println!(
                "repository: {}",
                manifest.repository.as_deref().unwrap_or("(none)")
            );

            let roblox_target = roblox_target.is_some_and(|_| true);

            println!("target: {}", manifest.target);
            println!(
                "\tlib path: {}",
                manifest
                    .target
                    .lib_path()
                    .map_or("(none)".to_string(), |p| p.to_string())
            );

            match roblox_target {
                #[cfg(feature = "roblox")]
                true => {
                    println!("\tbuild files: {}", display_build_files.join(", "));
                }
                _ => {
                    println!(
                        "\tbin path: {}",
                        manifest
                            .target
                            .bin_path()
                            .map_or("(none)".to_string(), |p| p.to_string())
                    );
                }
            }

            println!(
                "includes: {}",
                display_includes.into_iter().collect::<Vec<_>>().join(", ")
            );

            if !self.dry_run && !inquire::Confirm::new("is this information correct?").prompt()? {
                println!("{}", "publish aborted".red().bold());

                return Ok(());
            }
        }

        let temp_manifest_path = project
            .data_dir()
            .join(format!("temp_manifest_{}", chrono::Utc::now().timestamp()));

        std::fs::write(
            &temp_manifest_path,
            toml::to_string(&manifest).context("failed to serialize manifest")?,
        )
        .context("failed to write temp manifest file")?;

        let mut temp_manifest = std::fs::File::open(&temp_manifest_path)
            .context("failed to open temp manifest file")?;

        archive.append_file(MANIFEST_FILE_NAME, &mut temp_manifest)?;

        drop(temp_manifest);

        std::fs::remove_file(temp_manifest_path)?;

        let archive = archive
            .into_inner()
            .context("failed to encode archive")?
            .finish()
            .context("failed to get archive bytes")?;

        if archive.len() > MAX_ARCHIVE_SIZE {
            anyhow::bail!(
                "archive size exceeds maximum size of {} bytes by {} bytes",
                MAX_ARCHIVE_SIZE,
                archive.len() - MAX_ARCHIVE_SIZE
            );
        }

        if self.dry_run {
            std::fs::write("package.tar.gz", archive)?;

            println!(
                "{}",
                "(dry run) package written to package.tar.gz".green().bold()
            );

            return Ok(());
        }

        todo!("publishing to registry");
    }
}
