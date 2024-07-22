use crate::cli::read_config;
use anyhow::Context;
use clap::Args;
use colored::Colorize;
use inquire::validator::Validation;
use pesde::{
    errors::ManifestReadError, manifest::ScriptName, names::PackageName, Project,
    DEFAULT_INDEX_NAME,
};
use std::{path::Path, str::FromStr};

#[derive(Debug, Args)]
pub struct InitCommand {}

fn script_contents(path: &Path) -> String {
    format!(
        concat!(
            r#"local process = require("@lune/process")   
local home_dir = if process.os == "windows" then process.env.userprofile else process.env.HOME

require(home_dir .. ""#,
            "/.",
            env!("CARGO_PKG_NAME"),
            r#"/scripts/{}")"#,
        ),
        path.display()
    )
}

impl InitCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        match project.read_manifest() {
            Ok(_) => {
                println!("{}", "project already initialized".red());
                return Ok(());
            }
            Err(ManifestReadError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        };

        let mut manifest = toml_edit::DocumentMut::new();

        manifest["name"] = toml_edit::value(
            inquire::Text::new("What is the name of the project?")
                .with_validator(|name: &str| {
                    Ok(match PackageName::from_str(name) {
                        Ok(_) => Validation::Valid,
                        Err(e) => Validation::Invalid(e.to_string().into()),
                    })
                })
                .prompt()
                .unwrap(),
        );
        manifest["version"] = toml_edit::value("0.1.0");

        let description =
            inquire::Text::new("What is the description of the project? (leave empty for none)")
                .prompt()
                .unwrap();

        if !description.is_empty() {
            manifest["description"] = toml_edit::value(description);
        }

        let authors = inquire::Text::new(
            "Who are the authors of this project? (leave empty for none, comma separated)",
        )
        .prompt()
        .unwrap();

        let authors = authors
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.into())
            .collect::<Vec<toml_edit::Value>>();

        if !authors.is_empty() {
            let mut authors_arr = toml_edit::Array::new();
            authors_arr.extend(authors);

            manifest["authors"] = toml_edit::value(authors_arr);
        }

        let repo = inquire::Text::new(
            "What is the repository URL of this project? (leave empty for none)",
        )
        .with_validator(|repo: &str| {
            if repo.is_empty() {
                return Ok(Validation::Valid);
            }

            Ok(match url::Url::parse(repo) {
                Ok(_) => Validation::Valid,
                Err(e) => Validation::Invalid(e.to_string().into()),
            })
        })
        .prompt()
        .unwrap();
        if !repo.is_empty() {
            manifest["repository"] = toml_edit::value(repo);
        }

        let license =
            inquire::Text::new("What is the license of this project? (leave empty for none)")
                .with_initial_value("MIT")
                .prompt()
                .unwrap();
        if !license.is_empty() {
            manifest["license"] = toml_edit::value(license);
        }

        let target_env = inquire::Select::new(
            "What environment are you targeting for your package?",
            vec![
                #[cfg(feature = "roblox")]
                "roblox",
                #[cfg(feature = "lune")]
                "lune",
                #[cfg(feature = "luau")]
                "luau",
            ],
        )
        .prompt()
        .unwrap();

        let mut target = toml_edit::Table::new();
        target["environment"] = toml_edit::value(target_env);

        if target_env == "roblox"
            || inquire::Confirm::new(&format!(
                "Would you like to setup a default {} script?",
                ScriptName::RobloxSyncConfigGenerator
            ))
            .prompt()
            .unwrap()
        {
            let folder = project.path().join(concat!(".", env!("CARGO_PKG_NAME")));
            std::fs::create_dir_all(&folder).context("failed to create scripts folder")?;

            std::fs::write(
                folder.join(format!("{}.luau", ScriptName::RobloxSyncConfigGenerator)),
                script_contents(Path::new(&format!(
                    "lune/rojo/{}.luau",
                    ScriptName::RobloxSyncConfigGenerator
                ))),
            )
            .context("failed to write script file")?;

            let scripts = manifest
                .entry("scripts")
                .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
                .as_table_mut()
                .unwrap();

            scripts[&ScriptName::RobloxSyncConfigGenerator.to_string()] =
                toml_edit::value(format!(
                    concat!(".", env!("CARGO_PKG_NAME"), "/{}.luau"),
                    ScriptName::RobloxSyncConfigGenerator
                ));
        }

        let mut indices = toml_edit::Table::new();
        indices[DEFAULT_INDEX_NAME] =
            toml_edit::value(read_config(project.data_dir())?.default_index.as_str());

        project.write_manifest(manifest.to_string())?;

        println!("{}", "initialized project".green());
        Ok(())
    }
}
