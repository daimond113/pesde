use std::path::{Component, Path};

use crate::manifest::target::TargetKind;
use full_moon::{ast::luau::ExportedTypeDeclaration, visitors::Visitor};
use relative_path::RelativePathBuf;

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

/// Get the types exported by a file
pub fn get_file_types(file: &str) -> Result<Vec<String>, Vec<full_moon::Error>> {
    let ast = full_moon::parse(file)?;
    let mut visitor = TypeVisitor { types: vec![] };
    visitor.visit_ast(&ast);

    Ok(visitor.types)
}

/// Generate a linking module for a library
pub fn generate_lib_linking_module<I: IntoIterator<Item = S>, S: AsRef<str>>(
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

fn luau_style_path(path: &Path) -> String {
    let path = path
        .components()
        .filter_map(|ct| match ct {
            Component::CurDir => Some(".".to_string()),
            Component::ParentDir => Some("..".to_string()),
            Component::Normal(part) => Some(format!("{}", part.to_string_lossy())),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");

    let require = format!("./{path}");
    format!("{require:?}")
}

/// Get the require path for a library
pub fn get_lib_require_path(
    target: &TargetKind,
    base_dir: &Path,
    lib_file: &RelativePathBuf,
    destination_dir: &Path,
    use_new_structure: bool,
) -> String {
    let path = pathdiff::diff_paths(destination_dir, base_dir).unwrap();
    let path = if use_new_structure {
        log::debug!("using new structure for require path");
        lib_file.to_path(path)
    } else {
        log::debug!("using old structure for require path");
        path
    };

    #[cfg(feature = "roblox")]
    if matches!(target, TargetKind::Roblox) {
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

        return format!("script.Parent{path}");
    };

    luau_style_path(&path)
}

/// Generate a linking module for a binary
pub fn generate_bin_linking_module(path: &str) -> String {
    format!("return require({path})")
}

/// Get the require path for a binary
pub fn get_bin_require_path(
    base_dir: &Path,
    bin_file: &RelativePathBuf,
    destination_dir: &Path,
) -> String {
    let path = pathdiff::diff_paths(destination_dir, base_dir).unwrap();
    let path = bin_file.to_path(path);

    luau_style_path(&path)
}
