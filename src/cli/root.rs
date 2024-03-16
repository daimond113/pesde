use std::{
    fs::{create_dir_all, read, remove_dir_all, write, File},
    str::FromStr,
    time::Duration,
};

use flate2::{write::GzEncoder, Compression};
use futures_executor::block_on;
use ignore::{overrides::OverrideBuilder, WalkBuilder};
use inquire::{validator::Validation, Select, Text};
use log::debug;
use lune::Runtime;
use reqwest::{header::AUTHORIZATION, Url};
use semver::Version;
use serde_json::Value;
use tar::Builder as TarBuilder;

use pesde::{
    dependencies::{registry::RegistryDependencySpecifier, DependencySpecifier, PackageRef},
    index::{GitIndex, Index},
    manifest::{Manifest, PathStyle, Realm},
    multithread::MultithreadedJob,
    package_name::PackageName,
    patches::{create_patch, setup_patches_repo},
    project::{InstallOptions, Project},
    DEV_PACKAGES_FOLDER, IGNORED_FOLDERS, MANIFEST_FILE_NAME, PACKAGES_FOLDER, PATCHES_FOLDER,
    SERVER_PACKAGES_FOLDER,
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

fn multithreaded_bar<E: Send + Sync + Into<anyhow::Error> + 'static>(
    params: &CliParams,
    job: MultithreadedJob<E>,
    len: u64,
    message: String,
) -> Result<(), anyhow::Error> {
    let bar = params.multi.add(
        indicatif::ProgressBar::new(len)
            .with_style(
                indicatif::ProgressStyle::default_bar()
                    .template("{msg} {bar:40.208/166} {pos}/{len} {percent}% {elapsed_precise}")?,
            )
            .with_message(message),
    );
    bar.enable_steady_tick(Duration::from_millis(100));

    while let Ok(result) = job.progress().recv() {
        result.map_err(Into::into)?;
        bar.inc(1);
    }

    bar.finish_with_message("done");

    Ok(())
}

macro_rules! none_if_empty {
    ($s:expr) => {
        if $s.is_empty() {
            None
        } else {
            Some($s)
        }
    };
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

            multithreaded_bar(
                &params,
                download_job,
                resolved_versions_map.len() as u64,
                "Downloading packages".to_string(),
            )?;

            project.install(
                InstallOptions::new()
                    .locked(locked)
                    .auto_download(false)
                    .resolved_versions_map(resolved_versions_map),
            )?;
        }
        Command::Run { package, args } => {
            let project = get_project(&params)?;

            let lockfile = project
                .lockfile()?
                .ok_or(anyhow::anyhow!("lockfile not found"))?;

            let (_, resolved_pkg) = lockfile
                .get(&package)
                .and_then(|versions| versions.iter().find(|(_, pkg_ref)| pkg_ref.is_root))
                .ok_or(anyhow::anyhow!(
                    "package not found in lockfile (or isn't root)"
                ))?;

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

            let response = send_request(params.reqwest_client.get(Url::parse_with_params(
                &format!("{}/v0/search", api_url),
                &query.map(|q| vec![("query", q)]).unwrap_or_default(),
            )?))?
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
                        .unwrap_or_default()
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

            println!("{}", send_request(request)?.text()?);
        }
        Command::Patch { package } => {
            let project = get_project(&params)?;

            let lockfile = project
                .lockfile()?
                .ok_or(anyhow::anyhow!("lockfile not found"))?;

            let resolved_pkg = lockfile
                .get(&package.0)
                .and_then(|versions| versions.get(&package.1))
                .ok_or(anyhow::anyhow!("package not found in lockfile"))?;

            let dir = params.directories.data_dir().join("patches").join(format!(
                "{}_{}",
                package.0.escaped(),
                package.1
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
        Command::Init => {
            let manifest_path = params.cwd.join(MANIFEST_FILE_NAME);

            if manifest_path.exists() {
                anyhow::bail!("manifest already exists");
            }

            let default_name = params.cwd.file_name().unwrap().to_str().unwrap();

            let name = Text::new("What is the name of the package?")
                .with_initial_value(default_name)
                .with_validator(|name: &str| {
                    Ok(match PackageName::from_str(name) {
                        Ok(_) => Validation::Valid,
                        Err(e) => Validation::Invalid(e.into()),
                    })
                })
                .prompt()?;

            let path_style =
                Select::new("What style of paths do you want to use?", vec!["roblox"]).prompt()?;
            let path_style = match path_style {
                "roblox" => PathStyle::Roblox {
                    place: Default::default(),
                },
                _ => unreachable!(),
            };

            let description = Text::new("What is the description of the package?").prompt()?;
            let license = Text::new("What is the license of the package?").prompt()?;
            let authors = Text::new("Who are the authors of the package? (split using ;)")
                .prompt()?
                .split(';')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<String>>();
            let repository = Text::new("What is the repository of the package?").prompt()?;

            let private = Select::new("Is this package private?", vec!["yes", "no"]).prompt()?;
            let private = private == "yes";

            let realm = Select::new(
                "What is the realm of the package?",
                vec!["shared", "server", "dev"],
            )
            .prompt()?;

            let realm = match realm {
                "shared" => Realm::Shared,
                "server" => Realm::Server,
                "dev" => Realm::Development,
                _ => unreachable!(),
            };

            let manifest = Manifest {
                name: name.parse()?,
                version: Version::parse("0.1.0")?,
                exports: Default::default(),
                path_style,
                private,
                realm: Some(realm),
                dependencies: Default::default(),
                peer_dependencies: Default::default(),
                description: none_if_empty!(description),
                license: none_if_empty!(license),
                authors: none_if_empty!(authors),
                repository: none_if_empty!(repository),
            };

            serde_yaml::to_writer(File::create(manifest_path)?, &manifest)?;
        }
        Command::Add {
            package,
            realm,
            peer,
        } => {
            let project = get_project(&params)?;

            let mut manifest = project.manifest().clone();

            let specifier = DependencySpecifier::Registry(RegistryDependencySpecifier {
                name: package.0,
                version: package.1,
                realm,
            });

            if peer {
                manifest.peer_dependencies.push(specifier);
            } else {
                manifest.dependencies.push(specifier);
            }

            serde_yaml::to_writer(
                File::create(project.path().join(MANIFEST_FILE_NAME))?,
                &manifest,
            )?;
        }
        Command::Remove { package } => {
            let project = get_project(&params)?;

            let mut manifest = project.manifest().clone();

            for dependencies in [&mut manifest.dependencies, &mut manifest.peer_dependencies] {
                dependencies.retain(|d| {
                    if let DependencySpecifier::Registry(registry) = d {
                        registry.name != package
                    } else {
                        true
                    }
                });
            }

            serde_yaml::to_writer(
                File::create(project.path().join(MANIFEST_FILE_NAME))?,
                &manifest,
            )?;
        }
        Command::Outdated => {
            let project = get_project(&params)?;

            let manifest = project.manifest();
            let dependency_tree = manifest.dependency_tree(&project, false)?;

            for (name, versions) in dependency_tree {
                for (version, resolved_pkg) in versions {
                    if !resolved_pkg.is_root {
                        continue;
                    }

                    if let PackageRef::Registry(registry) = resolved_pkg.pkg_ref {
                        let latest_version = send_request(params.reqwest_client.get(format!(
                            "{}/v0/packages/{}/{}/versions",
                            project.index().config()?.api(),
                            registry.name.scope(),
                            registry.name.name()
                        )))?
                        .json::<Value>()?
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| Version::parse(v.as_str().unwrap()))
                        .collect::<Result<Vec<Version>, semver::Error>>()?
                        .into_iter()
                        .max()
                        .unwrap();

                        if latest_version > version {
                            println!(
                                "{name}@{version} is outdated. latest version: {latest_version}"
                            );
                        }
                    }
                }
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}
