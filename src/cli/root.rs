use std::{
    fs::{create_dir_all, read, remove_dir_all, write},
    time::Duration,
};

use flate2::{write::GzEncoder, Compression};
use futures_executor::block_on;
use ignore::{overrides::OverrideBuilder, WalkBuilder};
use log::debug;
use lune::Runtime;
use reqwest::{header::AUTHORIZATION, Url};
use semver::Version;
use serde_json::Value;
use tar::Builder as TarBuilder;

use pesde::{
    dependencies::PackageRef,
    index::{GitIndex, Index},
    manifest::Manifest,
    package_name::PackageName,
    patches::{create_patch, setup_patches_repo},
    project::{InstallOptions, Project},
    DEV_PACKAGES_FOLDER, IGNORED_FOLDERS, PACKAGES_FOLDER, PATCHES_FOLDER, SERVER_PACKAGES_FOLDER,
};

use crate::{send_request, CliParams, Command};

pub const MAX_ARCHIVE_SIZE: usize = 4 * 1024 * 1024;

fn get_project(params: &CliParams) -> anyhow::Result<Project<GitIndex>> {
    Project::from_path(
        &params.cwd,
        params.cli_config.cache_dir(&params.directories),
        params.index.clone(),
        params.api_token_entry.get_password().ok(),
    )
    .map_err(Into::into)
}

pub fn root_command(cmd: Command, params: CliParams) -> anyhow::Result<()> {
    match cmd {
        Command::Install { locked } => {
            let project = get_project(&params)?;

            for packages_folder in &[PACKAGES_FOLDER, DEV_PACKAGES_FOLDER, SERVER_PACKAGES_FOLDER] {
                if let Err(e) = remove_dir_all(&params.cwd.join(packages_folder)) {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        return Err(e.into());
                    } else {
                        debug!("no {packages_folder} folder found, skipping removal");
                    }
                };
            }

            let resolved_versions_map = project.manifest().dependency_tree(&project, locked)?;

            let download_job = project.download(&resolved_versions_map)?;
            let bar = params.multi.add(
                indicatif::ProgressBar::new(resolved_versions_map.len() as u64)
                    .with_style(indicatif::ProgressStyle::default_bar().template(
                        "{msg} {bar:40.208/166} {pos}/{len} {percent}% {elapsed_precise}",
                    )?)
                    .with_message("Downloading packages"),
            );
            bar.enable_steady_tick(Duration::from_millis(100));

            while let Ok(result) = download_job.progress().recv() {
                result?;
                bar.inc(1);
            }

            bar.finish_with_message("done");

            project.install(
                InstallOptions::new()
                    .locked(locked)
                    .auto_download(false)
                    .resolved_versions_map(resolved_versions_map),
            )?;
        }
        Command::Run { package, args } => {
            let project = get_project(&params)?;

            let name: PackageName = package.parse()?;

            let lockfile = project
                .lockfile()?
                .ok_or(anyhow::anyhow!("lockfile not found"))?;

            let (_, resolved_pkg) = lockfile
                .get(&name)
                .and_then(|versions| versions.iter().find(|(_, pkg_ref)| pkg_ref.is_root))
                .ok_or(anyhow::anyhow!("package not found in lockfile"))?;

            if !resolved_pkg.is_root {
                anyhow::bail!("package is not a root package");
            }

            let pkg_path = resolved_pkg.directory(project.path()).1;
            let manifest = Manifest::from_path(&pkg_path)?;

            let Some(bin_path) = manifest.exports.bin else {
                anyhow::bail!("no bin found in package");
            };

            let absolute_bin_path = bin_path.to_path(pkg_path);

            let mut runtime = Runtime::new().with_args(args);

            block_on(runtime.run(
                resolved_pkg.pkg_ref.name().to_string(),
                &read(absolute_bin_path)?,
            ))?;
        }
        Command::Search { query } => {
            let config = params.index.config()?;
            let api_url = config.api();

            let response = send_request!(params.reqwest_client.get(Url::parse_with_params(
                &format!("{}/v0/search", api_url),
                &query.map_or_else(Vec::new, |q| vec![("query", q)])
            )?))
            .json::<Value>()?;

            for package in response.as_array().unwrap() {
                println!(
                    "{}@{}{}",
                    package["name"].as_str().unwrap(),
                    package["version"].as_str().unwrap(),
                    package["description"]
                        .as_str()
                        .map(|d| if d.is_empty() {
                            d.to_string()
                        } else {
                            format!("\n{}\n", d)
                        })
                        .unwrap()
                );
            }
        }
        Command::Publish => {
            let project = get_project(&params)?;

            if project.manifest().private {
                anyhow::bail!("package is private, cannot publish");
            }

            let encoder = GzEncoder::new(vec![], Compression::default());
            let mut archive = TarBuilder::new(encoder);

            let mut walk_builder = WalkBuilder::new(&params.cwd);
            walk_builder.add_custom_ignore_filename(".pesdeignore");
            let mut overrides = OverrideBuilder::new(&params.cwd);

            for packages_folder in IGNORED_FOLDERS {
                overrides.add(&format!("!{}", packages_folder))?;
            }

            walk_builder.overrides(overrides.build()?);

            for entry in walk_builder.build() {
                let entry = entry?;
                let path = entry.path();
                let relative_path = path.strip_prefix(&params.cwd)?;
                let entry_type = entry
                    .file_type()
                    .ok_or(anyhow::anyhow!("failed to get file type"))?;

                if relative_path.as_os_str().is_empty() {
                    continue;
                }

                if entry_type.is_file() {
                    archive.append_path_with_name(path, relative_path)?;
                } else if entry_type.is_dir() {
                    archive.append_dir(relative_path, path)?;
                }
            }

            let archive = archive.into_inner()?.finish()?;

            if archive.len() > MAX_ARCHIVE_SIZE {
                anyhow::bail!(
                    "archive is too big ({} bytes), max {MAX_ARCHIVE_SIZE}. aborting...",
                    archive.len()
                );
            }

            let part = reqwest::blocking::multipart::Part::bytes(archive)
                .file_name("tarball.tar.gz")
                .mime_str("application/gzip")?;

            let mut request = params
                .reqwest_client
                .post(format!("{}/v0/packages", project.index().config()?.api()))
                .multipart(reqwest::blocking::multipart::Form::new().part("tarball", part));

            if let Some(token) = project.registry_auth_token() {
                request = request.header(AUTHORIZATION, format!("Bearer {token}"));
            } else {
                request = request.header(AUTHORIZATION, "");
            }

            println!("{}", send_request!(request).text()?);
        }
        Command::Patch { package } => {
            let project = get_project(&params)?;

            let (name, version) = package
                .split_once('@')
                .ok_or(anyhow::anyhow!("Malformed package name"))?;
            let name: PackageName = name.parse()?;
            let version = Version::parse(version)?;

            let lockfile = project
                .lockfile()?
                .ok_or(anyhow::anyhow!("lockfile not found"))?;

            let resolved_pkg = lockfile
                .get(&name)
                .and_then(|versions| versions.get(&version))
                .ok_or(anyhow::anyhow!("package not found in lockfile"))?;

            let dir = params.directories.data_dir().join("patches").join(format!(
                "{}_{}",
                name.escaped(),
                version
            ));

            if dir.exists() {
                anyhow::bail!(
                    "patch already exists. remove the directory {} to create a new patch",
                    dir.display()
                );
            }

            create_dir_all(&dir)?;

            resolved_pkg.pkg_ref.download(&project, &dir)?;
            match resolved_pkg.pkg_ref {
                PackageRef::Git(_) => {}
                _ => {
                    setup_patches_repo(&dir)?;
                }
            }

            println!("done! modify the files in {} and run `{} patch-commit <DIRECTORY>` to commit the changes", dir.display(), env!("CARGO_BIN_NAME"));
        }
        Command::PatchCommit { dir } => {
            let project = get_project(&params)?;

            let manifest = Manifest::from_path(&dir)?;
            let patch_path = project.path().join(PATCHES_FOLDER).join(format!(
                "{}@{}.patch",
                manifest.name.escaped(),
                manifest.version
            ));

            if patch_path.exists() {
                anyhow::bail!(
                    "patch already exists. remove the file {} to create a new patch",
                    patch_path.display()
                );
            }

            let patches = create_patch(&dir)?;

            write(&patch_path, patches)?;

            remove_dir_all(&dir)?;

            println!(
                "done! to apply the patch, run `{} install`",
                env!("CARGO_BIN_NAME")
            );
        }
        _ => unreachable!(),
    }

    Ok(())
}
