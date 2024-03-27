use std::{collections::BTreeMap, fmt::Display, fs::read, str::FromStr};

use cfg_if::cfg_if;
use relative_path::RelativePathBuf;
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    dependencies::DependencySpecifier, package_name::StandardPackageName, MANIFEST_FILE_NAME,
};

/// The files exported by the package
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct Exports {
    /// Points to the file which exports the package. As of currently this is only used for re-exporting types.
    /// Libraries must have a structure in Roblox where the main file becomes the folder, for example:
    /// A package called pesde/lib has a file called src/main.lua.
    /// pesde puts this package in a folder called pesde_lib.
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
    pub fn or(self, other: Self) -> Self {
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

/// An error that occurred while parsing a realm from a string
#[derive(Debug, Error)]
#[error("invalid realm {0}")]
pub struct FromStrRealmError(String);

impl FromStr for Realm {
    type Err = FromStrRealmError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "shared" => Ok(Realm::Shared),
            "server" => Ok(Realm::Server),
            "development" => Ok(Realm::Development),
            _ => Err(FromStrRealmError(s.to_string())),
        }
    }
}

/// A key to override dependencies
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OverrideKey(pub Vec<Vec<String>>);

impl Serialize for OverrideKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(
            &self
                .0
                .iter()
                .map(|overrides| {
                    overrides
                        .iter()
                        .map(String::to_string)
                        .collect::<Vec<_>>()
                        .join(">")
                })
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

impl<'de> Deserialize<'de> for OverrideKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let mut key = Vec::new();
        for overrides in s.split(',') {
            key.push(
                overrides
                    .split('>')
                    .map(|s| String::from_str(s).map_err(serde::de::Error::custom))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }

        Ok(OverrideKey(key))
    }
}

/// The manifest of a package
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Manifest {
    /// The name of the package
    pub name: StandardPackageName,
    /// The version of the package. Must be [semver](https://semver.org) compatible. The registry will not accept non-semver versions and the CLI will not handle such packages
    pub version: Version,
    /// A short description of the package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The license of the package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// The authors of the package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    /// The repository of the package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
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
    /// Indices of the package
    pub indices: BTreeMap<String, String>,
    /// The command to generate a `sourcemap.json`
    #[cfg(feature = "wally")]
    #[serde(default)]
    pub sourcemap_generator: Option<String>,
    /// Dependency overrides
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<OverrideKey, DependencySpecifier>,

    /// The dependencies of the package
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, DependencySpecifier>,
    /// The peer dependencies of the package
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub peer_dependencies: BTreeMap<String, DependencySpecifier>,
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

cfg_if! {
    if #[cfg(feature = "wally")] {
        /// An error that occurred while converting the manifest
        #[derive(Debug, Error)]
        pub enum ManifestConvertError {
            /// An error that occurred while reading the manifest
            #[error("error reading the manifest")]
            ManifestRead(#[from] ManifestReadError),

            /// An error that occurred while converting the manifest
            #[error("error converting the manifest")]
            ManifestConvert(#[source] toml::de::Error),

            /// The given path does not have a parent
            #[error("the path {0} does not have a parent")]
            NoParent(std::path::PathBuf),

            /// An error that occurred while interacting with the file system
            #[error("error interacting with the file system")]
            Io(#[from] std::io::Error),

            /// An error that occurred while making a package name from a string
            #[error("error making a package name from a string")]
            PackageName(
                #[from]
                crate::package_name::FromStrPackageNameParseError<
                    crate::package_name::StandardPackageNameValidationError,
                >,
            ),

            /// An error that occurred while writing the manifest
            #[error("error writing the manifest")]
            ManifestWrite(#[from] serde_yaml::Error),

            /// An error that occurred while parsing the dependencies
            #[error("error parsing the dependencies")]
            DependencyParse(#[from] crate::dependencies::wally::WallyManifestDependencyError),
        }
    } else {
        /// An error that occurred while converting the manifest
        pub type ManifestConvertError = ManifestReadError;
    }
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

    /// Tries to read the manifest from the given path, and if it fails, tries converting the `wally.toml` and writes a `pesde.yaml` in the same directory
    #[cfg(feature = "wally")]
    pub fn from_path_or_convert<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, ManifestConvertError> {
        let dir_path = if path.as_ref().file_name() == Some(MANIFEST_FILE_NAME.as_ref()) {
            path.as_ref()
                .parent()
                .ok_or_else(|| ManifestConvertError::NoParent(path.as_ref().to_path_buf()))?
                .to_path_buf()
        } else {
            path.as_ref().to_path_buf()
        };

        Self::from_path(path).or_else(|_| {
            let toml_path = dir_path.join("wally.toml");
            let toml_contents = std::fs::read_to_string(toml_path)?;
            let wally_manifest: crate::dependencies::wally::WallyManifest =
                toml::from_str(&toml_contents).map_err(ManifestConvertError::ManifestConvert)?;

            let dependencies =
                crate::dependencies::wally::parse_wally_dependencies(wally_manifest.clone())?;

            let mut place = BTreeMap::new();

            if let Some(shared) = wally_manifest.place.shared_packages {
                if !shared.is_empty() {
                    place.insert(Realm::Shared, shared);
                }
            }

            if let Some(server) = wally_manifest.place.server_packages {
                if !server.is_empty() {
                    place.insert(Realm::Server, server);
                }
            }

            let manifest = Self {
                name: wally_manifest.package.name.replace('-', "_").parse()?,
                version: wally_manifest.package.version,
                exports: Exports {
                    lib: Some(RelativePathBuf::from("true")),
                    bin: None,
                },
                path_style: PathStyle::Roblox { place },
                private: wally_manifest.package.private.unwrap_or(false),
                realm: wally_manifest
                    .package
                    .realm
                    .map(|r| r.parse().unwrap_or(Realm::Shared)),
                indices: BTreeMap::from([(
                    crate::project::DEFAULT_INDEX_NAME.to_string(),
                    "".to_string(),
                )]),
                sourcemap_generator: None,
                overrides: BTreeMap::new(),

                dependencies,
                peer_dependencies: Default::default(),
                description: wally_manifest.package.description,
                license: wally_manifest.package.license,
                authors: wally_manifest.package.authors,
                repository: None,
            };

            let manifest_path = dir_path.join(MANIFEST_FILE_NAME);
            serde_yaml::to_writer(std::fs::File::create(manifest_path)?, &manifest)?;

            Ok(manifest)
        })
    }

    /// Same as `from_path`, enable the `wally` feature to add support for converting `wally.toml` to `pesde.yaml`
    #[cfg(not(feature = "wally"))]
    pub fn from_path_or_convert<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, ManifestReadError> {
        Self::from_path(path)
    }

    /// Returns all dependencies
    pub fn dependencies(&self) -> BTreeMap<String, (DependencySpecifier, DependencyType)> {
        self.dependencies
            .iter()
            .map(|(desired_name, specifier)| {
                (
                    desired_name.clone(),
                    (specifier.clone(), DependencyType::Normal),
                )
            })
            .chain(
                self.peer_dependencies
                    .iter()
                    .map(|(desired_name, specifier)| {
                        (
                            desired_name.clone(),
                            (specifier.clone(), DependencyType::Peer),
                        )
                    }),
            )
            .collect()
    }
}
