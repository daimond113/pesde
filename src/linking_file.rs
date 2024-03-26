use std::{
    collections::HashSet,
    fs::{read_to_string, write},
    path::{Component, Path, PathBuf},
};

use full_moon::{
    ast::types::ExportedTypeDeclaration,
    parse,
    visitors::{Visit, Visitor},
};
use log::debug;
use semver::Version;
use thiserror::Error;

use crate::{
    dependencies::resolution::{packages_folder, ResolvedPackage, RootLockfileNode},
    manifest::{Manifest, ManifestReadError, PathStyle, Realm},
    package_name::PackageName,
    project::Project,
};

struct TypeVisitor {
    pub(crate) types: Vec<String>,
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

/// Generates the contents of a linking file, given the require path, and the contents of the target file
/// The contents will be scanned for type exports, and the linking file will be generated accordingly
pub fn linking_file(content: &str, path: &str) -> Result<String, full_moon::Error> {
    let mut linker = format!("local module = require({path})\n");
    let mut visitor = TypeVisitor { types: vec![] };

    parse(content)?.nodes().visit(&mut visitor);

    for ty in visitor.types {
        linker.push_str(&ty);
    }

    linker.push_str("return module");

    Ok(linker)
}

#[derive(Debug, Error)]
/// An error that occurred while linking dependencies
pub enum LinkingError {
    #[error("error interacting with the file system")]
    /// An error that occurred while interacting with the file system
    Io(#[from] std::io::Error),

    #[error("failed getting file name from {0}")]
    /// An error that occurred while getting a file name
    FileNameFail(PathBuf),

    #[error("failed converting file name to string")]
    /// An error that occurred while converting a file name to a string
    FileNameToStringFail,

    #[error("failed getting relative path from {0} to {1}")]
    /// An error that occurred while getting a relative path
    RelativePathFail(PathBuf, PathBuf),

    #[error("failed getting path parent of {0}")]
    /// An error that occurred while getting a path parent
    ParentFail(PathBuf),

    #[error("failed to convert path component to string")]
    /// An error that occurred while converting a path component to a string
    ComponentToStringFail,

    #[error("failed to get path string")]
    /// An error that occurred while getting a path string
    PathToStringFail,

    #[error("error encoding utf-8 string")]
    /// An error that occurred while converting a byte slice to a string
    Utf8(#[from] std::str::Utf8Error),

    #[error("error reading manifest")]
    /// An error that occurred while reading the manifest of a package
    ManifestRead(#[from] ManifestReadError),

    #[error("missing realm {0} in-game path")]
    /// An error that occurred while getting the in-game path for a realm
    MissingRealmInGamePath(Realm),

    #[error("library source is not valid Luau")]
    /// An error that occurred because the library source is not valid Luau
    InvalidLuau(#[from] full_moon::Error),
}

pub(crate) fn link<P: AsRef<Path>, Q: AsRef<Path>>(
    project: &Project,
    resolved_pkg: &ResolvedPackage,
    lockfile: &RootLockfileNode,
    destination_dir: P,
    parent_dependency_packages_dir: Q,
    only_name: bool,
    as_root: bool,
) -> Result<(), LinkingError> {
    let (_, source_dir) = resolved_pkg.directory(project.path());
    let file = Manifest::from_path(&source_dir)?;

    let Some(relative_lib_export) = file.exports.lib else {
        return Ok(());
    };

    let lib_export = relative_lib_export.to_path(&source_dir);

    let path_style = &project.manifest().path_style;
    let PathStyle::Roblox { place } = &path_style;

    debug!("linking {resolved_pkg} using `{}` path style", path_style);

    let pkg_name = resolved_pkg.pkg_ref.name();
    let name = pkg_name.name();

    let destination_dir = match lockfile
        .specifiers
        .get(&pkg_name)
        .and_then(|v| v.get(resolved_pkg.pkg_ref.version()))
    {
        Some(specifier) if as_root => project.path().join(packages_folder(
            specifier.realm().copied().unwrap_or_default(),
        )),
        _ => destination_dir.as_ref().to_path_buf(),
    };

    let destination_file = destination_dir.join(format!(
        "{}{}.lua",
        if only_name { "" } else { pkg_name.prefix() },
        name
    ));

    let realm_folder = project.path().join(resolved_pkg.packages_folder());
    let in_different_folders = realm_folder != parent_dependency_packages_dir.as_ref();

    let mut path = if in_different_folders {
        pathdiff::diff_paths(&source_dir, &realm_folder)
            .ok_or_else(|| LinkingError::RelativePathFail(source_dir.clone(), realm_folder))?
    } else {
        pathdiff::diff_paths(&source_dir, &destination_dir).ok_or_else(|| {
            LinkingError::RelativePathFail(source_dir.clone(), destination_dir.to_path_buf())
        })?
    };
    path.set_extension("");

    let beginning = if in_different_folders {
        place
            .get(&resolved_pkg.realm)
            .ok_or_else(|| LinkingError::MissingRealmInGamePath(resolved_pkg.realm))?
            .clone()
    } else if name == "init" {
        "script".to_string()
    } else {
        "script.Parent".to_string()
    };

    let mut components = path
        .components()
        .map(|component| {
            Ok(match component {
                Component::ParentDir => ".Parent".to_string(),
                Component::Normal(part) => format!(
                    "[{:?}]",
                    part.to_str().ok_or(LinkingError::ComponentToStringFail)?
                ),
                _ => unreachable!("invalid path component"),
            })
        })
        .collect::<Result<Vec<_>, LinkingError>>()?;
    components.pop();

    let path = beginning + &components.join("") + &format!("[{name:?}]");

    debug!(
        "writing linking file for {} with import `{path}` to {}",
        source_dir.display(),
        destination_file.display()
    );

    let file_contents = match relative_lib_export.as_str() {
        "true" => "".to_string(),
        _ => read_to_string(lib_export)?,
    };

    let linking_file_contents = linking_file(&file_contents, &path)?;

    write(&destination_file, linking_file_contents)?;

    Ok(())
}

#[derive(Debug, Error)]
#[error("error linking {1}@{2} to {3}@{4}")]
/// An error that occurred while linking the dependencies
pub struct LinkingDependenciesError(
    #[source] LinkingError,
    PackageName,
    Version,
    PackageName,
    Version,
);

fn is_duplicate_in<T: PartialEq>(item: T, items: &[T]) -> bool {
    let mut count = 0u8;
    items.iter().any(|i| {
        if i == &item {
            count += 1;
        }
        count > 1
    })
}

impl Project {
    /// Links the dependencies of the project
    pub fn link_dependencies(
        &self,
        lockfile: &RootLockfileNode,
    ) -> Result<(), LinkingDependenciesError> {
        let root_deps = lockfile.specifiers.keys().collect::<HashSet<_>>();
        let root_dep_names = root_deps.iter().map(|n| n.name()).collect::<Vec<_>>();

        for (name, versions) in &lockfile.children {
            for (version, resolved_pkg) in versions {
                let (container_dir, _) = resolved_pkg.directory(self.path());

                debug!(
                    "linking package {name}@{version}'s dependencies to directory {}",
                    container_dir.display()
                );

                let resolved_pkg_dep_names = resolved_pkg
                    .dependencies
                    .iter()
                    .map(|(n, _)| n.name())
                    .collect::<Vec<_>>();

                for (dep_name, dep_version) in &resolved_pkg.dependencies {
                    let dep = lockfile
                        .children
                        .get(dep_name)
                        .and_then(|versions| versions.get(dep_version))
                        .unwrap();

                    link(
                        self,
                        dep,
                        lockfile,
                        &container_dir,
                        &self.path().join(resolved_pkg.packages_folder()),
                        !is_duplicate_in(dep_name.name(), &resolved_pkg_dep_names),
                        false,
                    )
                    .map_err(|e| {
                        LinkingDependenciesError(
                            e,
                            dep_name.clone(),
                            dep_version.clone(),
                            name.clone(),
                            version.clone(),
                        )
                    })?;
                }

                if root_deps.contains(&name) {
                    let specifier = lockfile.root_specifier(resolved_pkg).unwrap();
                    let linking_dir = &self.path().join(packages_folder(
                        specifier.realm().copied().unwrap_or_default(),
                    ));

                    debug!(
                        "linking root package {name}@{version} to directory {}",
                        linking_dir.display()
                    );

                    link(
                        self,
                        resolved_pkg,
                        lockfile,
                        linking_dir,
                        self.path().join(resolved_pkg.packages_folder()),
                        !is_duplicate_in(name.name(), &root_dep_names),
                        true,
                    )
                    .map_err(|e| {
                        LinkingDependenciesError(
                            e,
                            name.clone(),
                            version.clone(),
                            name.clone(),
                            version.clone(),
                        )
                    })?;
                }
            }
        }

        Ok(())
    }
}
