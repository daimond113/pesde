use crate::cli::read_config;
use clap::Args;
use colored::Colorize;
use inquire::validator::Validation;
use pesde::{errors::ManifestReadError, names::PackageName, Project, DEFAULT_INDEX_NAME};
use std::str::FromStr;

#[derive(Debug, Args)]
pub struct InitCommand {}

impl InitCommand {
    pub fn run(self, project: Project) -> anyhow::Result<()> {
        match project.read_manifest() {
            Ok(_) => {
                println!("{}", "project already initialized".red());
                Ok(())
            }
            Err(ManifestReadError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                let mut manifest = nondestructive::yaml::from_slice(b"").unwrap();
                let mut mapping = manifest.as_mut().make_mapping();

                mapping.insert_str(
                    "name",
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
                mapping.insert_str("version", "0.1.0");

                let description = inquire::Text::new(
                    "What is the description of the project? (leave empty for none)",
                )
                .prompt()
                .unwrap();

                if !description.is_empty() {
                    mapping.insert_str("description", description);
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
                    .collect::<Vec<_>>();

                if !authors.is_empty() {
                    let mut authors_field = mapping
                        .insert("authors", nondestructive::yaml::Separator::Auto)
                        .make_sequence();

                    for author in authors {
                        authors_field.push_string(author);
                    }
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
                    mapping.insert_str("repository", repo);
                }

                let license = inquire::Text::new(
                    "What is the license of this project? (leave empty for none)",
                )
                .with_initial_value("MIT")
                .prompt()
                .unwrap();
                if !license.is_empty() {
                    mapping.insert_str("license", license);
                }

                let mut indices = mapping
                    .insert("indices", nondestructive::yaml::Separator::Auto)
                    .make_mapping();
                indices.insert_str(
                    DEFAULT_INDEX_NAME,
                    read_config(project.data_dir())?.default_index.as_str(),
                );

                project.write_manifest(manifest.to_string())?;

                println!("{}", "initialized project".green());
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }
}
