use std::path::{Component, Path};

use full_moon::{ast::luau::ExportedTypeDeclaration, visitors::Visitor};

use crate::manifest::Target;

struct TypeVisitor {
    types: Vec<String>,
}

impl Visitor for TypeVisitor {
    fn visit_exported_type_declaration(&mut self, node: &ExportedTypeDeclaration) {
        let name = node.type_declaration().type_name().to_string();

        let (declaration_generics, generics) =
            if let Some(declaration) = node.type_declaration().generics() {
                let mut declaration_generics = vec![];
                let mut generics = vec![];

                for generic in declaration.generics().iter() {
                    declaration_generics.push(generic.to_string());

                    if generic.default_type().is_some() {
                        generics.push(generic.parameter().to_string())
                    } else {
                        generics.push(generic.to_string())
                    }
                }

                (
                    format!("<{}>", declaration_generics.join(", ")),
                    format!("<{}>", generics.join(", ")),
                )
            } else {
                ("".to_string(), "".to_string())
            };

        self.types.push(format!(
            "export type {name}{declaration_generics} = module.{name}{generics}\n"
        ));
    }
}

pub fn get_file_types(file: &str) -> Result<Vec<String>, Vec<full_moon::Error>> {
    let ast = full_moon::parse(file)?;
    let mut visitor = TypeVisitor { types: vec![] };
    visitor.visit_ast(&ast);

    Ok(visitor.types)
}

pub fn generate_linking_module<I: IntoIterator<Item = S>, S: AsRef<str>>(
    path: &str,
    types: I,
) -> String {
    let mut output = format!("local module = require({path})\n");

    for ty in types {
        output.push_str(ty.as_ref());
    }

    output.push_str("return module");

    output
}

pub fn get_require_path(
    target: &Target,
    base_dir: &Path,
    destination_dir: &Path,
    use_new_structure: bool,
) -> Result<String, errors::GetRequirePathError> {
    let Some(lib_file) = target.lib_path() else {
        return Err(errors::GetRequirePathError::NoLibPath);
    };

    let path = pathdiff::diff_paths(destination_dir, base_dir).unwrap();
    let path = if !use_new_structure {
        lib_file.to_path(path)
    } else {
        path
    };

    #[cfg(feature = "roblox")]
    if matches!(target, Target::Roblox { .. }) {
        let path = path
            .components()
            .filter_map(|component| match component {
                Component::ParentDir => Some(".Parent".to_string()),
                Component::Normal(part) if part != "init.lua" && part != "init.luau" => {
                    Some(format!(
                        "[{:?}]",
                        part.to_string_lossy()
                            .trim_end_matches(".lua")
                            .trim_end_matches(".luau")
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        return Ok(format!("script{path}"));
    };

    let path = path
        .components()
        .filter_map(|ct| match ct {
            Component::ParentDir => Some("..".to_string()),
            Component::Normal(part) => Some(format!("{}", part.to_string_lossy())),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");

    Ok(format!("./{path}"))
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum GetRequirePathError {
        #[error("get require path called for target without a lib path")]
        NoLibPath,
    }
}
