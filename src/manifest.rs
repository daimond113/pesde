use std::{collections::BTreeMap, fmt::Display, fs::read};

use relative_path::RelativePathBuf;
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{dependencies::DependencySpecifier, package_name::PackageName, MANIFEST_FILE_NAME};

/// The files exported by the package
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct Exports {
    /// Points to the file which exports the package. As of currently this is only used for re-exporting types.
    /// Libraries must have a structure in Roblox where the main file becomes the folder, for example:
    /// A package called pesde/lib has a file called src/main.lua.
    /// Pesde puts this package in a folder called pesde_lib.
    /// The package has to have set up configuration for file-syncing tools such as Rojo so that src/main.lua becomes the pesde_lib and turns it into a ModuleScript
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lib: Option<RelativePathBuf>,

    /// Points to the file that will be executed with Lune
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<RelativePathBuf>,
}

/// The path style used by the package
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PathStyle {
    /// The path style used by Roblox (e.g. `script.Parent` or `script.Parent.Parent`)
    Roblox {
        /// A map of realm to in-game package folder location (used for linking between packages in different realms)
        #[serde(default)]
        place: BTreeMap<Realm, String>,
    },
}

impl Default for PathStyle {
    fn default() -> Self {
        PathStyle::Roblox {
            place: BTreeMap::new(),
        }
    }
}

impl Display for PathStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathStyle::Roblox { .. } => write!(f, "roblox"),
        }
    }
}

/// The realm of the package
#[derive(
    Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Copy, Default,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Realm {
    /// The package is shared (usually ReplicatedStorage)
    #[default]
    Shared,
    /// The package is server only (usually ServerScriptService/ServerStorage)
    Server,
    /// The package is development only
    Development,
}

impl Realm {
    /// Returns the most restrictive realm
    pub fn or<'a>(&'a self, other: &'a Self) -> &'a Self {
        match self {
            Realm::Shared => other,
            _ => self,
        }
    }
}

impl Display for Realm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Realm::Shared => write!(f, "shared"),
            Realm::Server => write!(f, "server"),
            Realm::Development => write!(f, "development"),
        }
    }
}

/// The manifest of a package
#[derive(Serialize, Deserialize, Debug, Clone)]
// #[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The name of the package
    pub name: PackageName,
    /// The version of the package. Must be [semver](https://semver.org) compatible. The registry will not accept non-semver versions and the CLI will not handle such packages
    pub version: Version,
    /// The files exported by the package
    #[serde(default)]
    pub exports: Exports,
    /// The path style to use for linking modules
    #[serde(default)]
    pub path_style: PathStyle,
    /// Whether the package is private (it should not be published)
    #[serde(default)]
    pub private: bool,
    /// The realm of the package
    pub realm: Option<Realm>,

    /// The dependencies of the package
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<DependencySpecifier>,
    /// The peer dependencies of the package
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub peer_dependencies: Vec<DependencySpecifier>,

    /// A short description of the package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The license of the package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// The authors of the package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
}

/// An error that occurred while reading the manifest
#[derive(Debug, Error)]
pub enum ManifestReadError {
    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while deserializing the manifest
    #[error("error deserializing manifest")]
    ManifestDeser(#[source] serde_yaml::Error),
}

/// The type of dependency
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    /// A normal dependency
    #[default]
    Normal,
    /// A peer dependency
    Peer,
}

impl Manifest {
    /// Reads a manifest from a path (if the path is a directory, it will look for the manifest file inside it, otherwise it will read the file directly)
    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Result<Self, ManifestReadError> {
        let path = path.as_ref();
        let path = if path.file_name() == Some(MANIFEST_FILE_NAME.as_ref()) {
            path.to_path_buf()
        } else {
            path.join(MANIFEST_FILE_NAME)
        };

        let raw_contents = read(path)?;
        let manifest =
            serde_yaml::from_slice(&raw_contents).map_err(ManifestReadError::ManifestDeser)?;

        Ok(manifest)
    }

    /// Returns all dependencies
    pub fn dependencies(&self) -> Vec<(DependencySpecifier, DependencyType)> {
        self.dependencies
            .iter()
            .map(|dep| (dep.clone(), DependencyType::Normal))
            .chain(
                self.peer_dependencies
                    .iter()
                    .map(|dep| (dep.clone(), DependencyType::Peer)),
            )
            .collect()
    }
}
