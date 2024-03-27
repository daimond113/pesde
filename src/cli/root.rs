use cfg_if::cfg_if;
use chrono::Utc;
use std::{
    collections::{BTreeMap, HashMap},
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
use once_cell::sync::Lazy;
use reqwest::{header::AUTHORIZATION, Url};
use semver::Version;
use serde_json::Value;
use tar::Builder as TarBuilder;

use pesde::{
    dependencies::{registry::RegistryDependencySpecifier, DependencySpecifier, PackageRef},
    index::Index,
    manifest::{Manifest, PathStyle, Realm},
    multithread::MultithreadedJob,
    package_name::{PackageName, StandardPackageName},
    patches::{create_patch, setup_patches_repo},
    project::{InstallOptions, Project, DEFAULT_INDEX_NAME},
    DEV_PACKAGES_FOLDER, IGNORED_FOLDERS, MANIFEST_FILE_NAME, PACKAGES_FOLDER, PATCHES_FOLDER,
    SERVER_PACKAGES_FOLDER,
};

use crate::cli::{
    clone_index, send_request, Command, CLI_CONFIG, CWD, DEFAULT_INDEX, DEFAULT_INDEX_URL, DIRS,
    MULTI, REQWEST_CLIENT,
};

pub const MAX_ARCHIVE_SIZE: usize = 4 * 1024 * 1024;

fn multithreaded_bar<E: Send + Sync + Into<anyhow::Error> + 'static>(
    job: MultithreadedJob<E>,
    len: u64,
    message: String,
) -> Result<(), anyhow::Error> {
    let bar = MULTI.add(
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

pub fn root_command(cmd: Command) -> anyhow::Result<()> {
    let mut project: Lazy<Project> = Lazy::new(|| {
        let manifest = Manifest::from_path(CWD.to_path_buf()).unwrap();
        let indices = manifest
            .indices
            .clone()
            .into_iter()
            .map(|(k, v)| (k, Box::new(clone_index(&v)) as Box<dyn Index>))
            .collect::<HashMap<_, _>>();

        Project::new(CWD.to_path_buf(), CLI_CONFIG.cache_dir(), indices, manifest).unwrap()
    });

    match cmd {
        Command::Install { locked } => {
            for packages_folder in &[PACKAGES_FOLDER, DEV_PACKAGES_FOLDER, SERVER_PACKAGES_FOLDER] {
                if let Err(e) = remove_dir_all(CWD.join(packages_folder)) {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        return Err(e.into());
                    } else {
                        debug!("no {packages_folder} folder found, skipping removal");
                    }
                };
            }

            let manifest = project.manifest().clone();
            let lockfile = manifest.dependency_graph(&mut project, locked)?;

            let download_job = project.download(&lockfile)?;

            multithreaded_bar(
                download_job,
                lockfile.children.len() as u64,
                "Downloading packages".to_string(),
            )?;

            #[allow(unused_variables)]
            project.convert_manifests(&lockfile, |path| {
                cfg_if! {
                    if #[cfg(feature = "wally")] {
                        if let Some(sourcemap_generator) = &manifest.sourcemap_generator {
                            cfg_if! {
                                if #[cfg(target_os = "windows")] {
                                    std::process::Command::new("pwsh")
                                        .args(["-C", &sourcemap_generator])
                                        .current_dir(path)
                                        .output()
                                        .expect("failed to execute process");
                                } else {
                                    std::process::Command::new("sh")
                                        .args(["-c", &sourcemap_generator])
                                        .current_dir(path)
                                        .output()
                                        .expect("failed to execute process");
                                }
                            }
                        }
                    }
                }
            })?;

            let project = Lazy::force_mut(&mut project);

            project.install(
                InstallOptions::new()
                    .locked(locked)
                    .auto_download(false)
                    .lockfile(lockfile),
            )?;
        }
        Command::Run { package, args } => {
            let lockfile = project
                .lockfile()?
                .ok_or(anyhow::anyhow!("lockfile not found"))?;

            let resolved_pkg = lockfile
                .children
                .get(&package.into())
                .and_then(|versions| {
                    versions
                        .values()
                        .find(|pkg_ref| lockfile.root_specifier(pkg_ref).is_some())
                })
                .ok_or(anyhow::anyhow!(
                    "package not found in lockfile (or isn't root)"
                ))?;

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
            let config = DEFAULT_INDEX.config()?;
            let api_url = config.api();

            let response = send_request(REQWEST_CLIENT.get(Url::parse_with_params(
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
            if project.manifest().private {
                anyhow::bail!("package is private, cannot publish");
            }

            let encoder = GzEncoder::new(vec![], Compression::default());
            let mut archive = TarBuilder::new(encoder);

            let cwd = &CWD.to_path_buf();

            let mut walk_builder = WalkBuilder::new(cwd);
            walk_builder.add_custom_ignore_filename(".pesdeignore");
            let mut overrides = OverrideBuilder::new(cwd);

            for packages_folder in IGNORED_FOLDERS {
                overrides.add(&format!("!{}", packages_folder))?;
            }

            walk_builder.overrides(overrides.build()?);

            for entry in walk_builder.build() {
                let entry = entry?;
                let path = entry.path();
                let relative_path = path.strip_prefix(cwd)?;
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

            let index = project.indices().get(DEFAULT_INDEX_NAME).unwrap();

            let mut request = REQWEST_CLIENT
                .post(format!("{}/v0/packages", index.config()?.api()))
                .multipart(reqwest::blocking::multipart::Form::new().part("tarball", part));

            if let Some(token) = index.registry_auth_token() {
                request = request.header(AUTHORIZATION, format!("Bearer {token}"));
            } else {
                request = request.header(AUTHORIZATION, "");
            }

            println!("{}", send_request(request)?.text()?);
        }
        Command::Patch { package } => {
            let lockfile = project
                .lockfile()?
                .ok_or(anyhow::anyhow!("lockfile not found"))?;

            let resolved_pkg = lockfile
                .children
                .get(&package.0)
                .and_then(|versions| versions.get(&package.1))
                .ok_or(anyhow::anyhow!("package not found in lockfile"))?;

            let dir = DIRS
                .data_dir()
                .join("patches")
                .join(package.0.escaped())
                .join(Utc::now().timestamp().to_string());

            if dir.exists() {
                anyhow::bail!(
                    "patch already exists. remove the directory {} to create a new patch",
                    dir.display()
                );
            }

            create_dir_all(&dir)?;

            let project = Lazy::force_mut(&mut project);
            let url = resolved_pkg.pkg_ref.resolve_url(project)?;

            let index = project.indices().get(DEFAULT_INDEX_NAME).unwrap();

            resolved_pkg.pkg_ref.download(
                &REQWEST_CLIENT,
                index.registry_auth_token().map(|t| t.to_string()),
                url.as_ref(),
                index.credentials_fn().cloned(),
                &dir,
            )?;

            match &resolved_pkg.pkg_ref {
                PackageRef::Git(_) => {}
                _ => {
                    setup_patches_repo(&dir)?;
                }
            }

            println!("done! modify the files in {} and run `{} patch-commit <DIRECTORY>` to commit the changes", dir.display(), env!("CARGO_BIN_NAME"));
        }
        Command::PatchCommit { dir } => {
            let name = dir
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|f| f.to_str())
                .unwrap();

            let manifest = Manifest::from_path(&dir)?;
            let patch_path = project.path().join(PATCHES_FOLDER);
            create_dir_all(&patch_path)?;

            let patch_path = patch_path.join(format!("{name}@{}.patch", manifest.version));
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
            let manifest_path = CWD.join(MANIFEST_FILE_NAME);

            if manifest_path.exists() {
                anyhow::bail!("manifest already exists");
            }

            let default_name = CWD.file_name().and_then(|s| s.to_str());

            let mut name =
                Text::new("What is the name of the package?").with_validator(|name: &str| {
                    Ok(match StandardPackageName::from_str(name) {
                        Ok(_) => Validation::Valid,
                        Err(e) => Validation::Invalid(e.into()),
                    })
                });

            if let Some(name_str) = default_name {
                name = name.with_initial_value(name_str);
            }

            let name = name.prompt()?;

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
                indices: BTreeMap::from([(
                    DEFAULT_INDEX_NAME.to_string(),
                    DEFAULT_INDEX_URL.to_string(),
                )]),
                #[cfg(feature = "wally")]
                sourcemap_generator: None,
                overrides: Default::default(),

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
            let mut manifest = project.manifest().clone();

            let specifier = match package.0.clone() {
                PackageName::Standard(name) => {
                    DependencySpecifier::Registry(RegistryDependencySpecifier {
                        name,
                        version: package.1,
                        realm,
                        index: DEFAULT_INDEX_NAME.to_string(),
                    })
                }
                #[cfg(feature = "wally")]
                PackageName::Wally(name) => DependencySpecifier::Wally(
                    pesde::dependencies::wally::WallyDependencySpecifier {
                        name,
                        version: package.1,
                        realm,
                        index_url: crate::cli::DEFAULT_WALLY_INDEX_URL.parse().unwrap(),
                    },
                ),
            };

            fn insert_into(
                deps: &mut BTreeMap<String, DependencySpecifier>,
                specifier: DependencySpecifier,
                name: PackageName,
            ) {
                macro_rules! not_taken {
                    ($key:expr) => {
                        (!deps.contains_key(&$key)).then_some($key)
                    };
                }

                let key = not_taken!(name.name().to_string())
                    .or_else(|| not_taken!(format!("{}/{}", name.scope(), name.name())))
                    .or_else(|| not_taken!(name.to_string()))
                    .unwrap();
                deps.insert(key, specifier);
            }

            if peer {
                insert_into(
                    &mut manifest.peer_dependencies,
                    specifier,
                    package.0.clone(),
                );
            } else {
                insert_into(&mut manifest.dependencies, specifier, package.0.clone());
            }

            serde_yaml::to_writer(
                File::create(project.path().join(MANIFEST_FILE_NAME))?,
                &manifest,
            )?;
        }
        Command::Remove { package } => {
            let mut manifest = project.manifest().clone();

            for dependencies in [&mut manifest.dependencies, &mut manifest.peer_dependencies] {
                dependencies.retain(|_, d| {
                    if let DependencySpecifier::Registry(registry) = d {
                        match &package {
                            PackageName::Standard(name) => &registry.name != name,
                            #[cfg(feature = "wally")]
                            PackageName::Wally(_) => true,
                        }
                    } else {
                        cfg_if! {
                            if #[cfg(feature = "wally")] {
                                #[allow(clippy::collapsible_else_if)]
                                if let DependencySpecifier::Wally(wally) = d {
                                    match &package {
                                        PackageName::Standard(_) => true,
                                        PackageName::Wally(name) => &wally.name != name,
                                    }
                                } else {
                                    true
                                }
                            } else {
                                true
                            }
                        }
                    }
                });
            }

            serde_yaml::to_writer(
                File::create(project.path().join(MANIFEST_FILE_NAME))?,
                &manifest,
            )?;
        }
        Command::Outdated => {
            let project = Lazy::force_mut(&mut project);

            let manifest = project.manifest().clone();
            let lockfile = manifest.dependency_graph(project, false)?;

            for (name, versions) in &lockfile.children {
                for (version, resolved_pkg) in versions {
                    if lockfile.root_specifier(resolved_pkg).is_none() {
                        continue;
                    }

                    if let PackageRef::Registry(registry) = &resolved_pkg.pkg_ref {
                        let latest_version = send_request(REQWEST_CLIENT.get(format!(
                            "{}/v0/packages/{}/{}/versions",
                            resolved_pkg.pkg_ref.get_index(project).config()?.api(),
                            registry.name.scope(),
                            registry.name.name()
                        )))?
                        .json::<Value>()?
                        .as_array()
                        .and_then(|a| a.last())
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<Version>().ok())
                        .ok_or(anyhow::anyhow!(
                            "failed to get latest version of {name}@{version}"
                        ))?;

                        if &latest_version > version {
                            println!(
                                "{name}@{version} is outdated. latest version: {latest_version}"
                            );
                        }
                    }
                }
            }
        }
        #[cfg(feature = "wally")]
        Command::Convert => {
            Manifest::from_path_or_convert(CWD.to_path_buf())?;
        }
        _ => unreachable!(),
    }

    Ok(())
}
