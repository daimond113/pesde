use relative_path::RelativePathBuf;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{
    collections::BTreeSet,
    fmt::{Display, Formatter},
    str::FromStr,
};

/// A kind of target
#[derive(
    SerializeDisplay, DeserializeFromStr, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum TargetKind {
    /// A Roblox target
    #[cfg(feature = "roblox")]
    Roblox,
    /// A Roblox server target
    #[cfg(feature = "roblox")]
    RobloxServer,
    /// A Lune target
    #[cfg(feature = "lune")]
    Lune,
    /// A Luau target
    #[cfg(feature = "luau")]
    Luau,
}

impl Display for TargetKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "roblox")]
            TargetKind::Roblox => write!(f, "roblox"),
            #[cfg(feature = "roblox")]
            TargetKind::RobloxServer => write!(f, "roblox_server"),
            #[cfg(feature = "lune")]
            TargetKind::Lune => write!(f, "lune"),
            #[cfg(feature = "luau")]
            TargetKind::Luau => write!(f, "luau"),
        }
    }
}

impl FromStr for TargetKind {
    type Err = errors::TargetKindFromStr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            #[cfg(feature = "roblox")]
            "roblox" => Ok(Self::Roblox),
            #[cfg(feature = "roblox")]
            "roblox_server" => Ok(Self::RobloxServer),
            #[cfg(feature = "lune")]
            "lune" => Ok(Self::Lune),
            #[cfg(feature = "luau")]
            "luau" => Ok(Self::Luau),
            t => Err(errors::TargetKindFromStr::Unknown(t.to_string())),
        }
    }
}

impl TargetKind {
    /// All possible target variants
    pub const VARIANTS: &'static [TargetKind] = &[
        #[cfg(feature = "roblox")]
        TargetKind::Roblox,
        #[cfg(feature = "roblox")]
        TargetKind::RobloxServer,
        #[cfg(feature = "lune")]
        TargetKind::Lune,
        #[cfg(feature = "luau")]
        TargetKind::Luau,
    ];

    /// Whether this target is compatible with another target
    /// self is the project's target, dependency is the target of the dependency
    pub fn is_compatible_with(&self, dependency: &Self) -> bool {
        if self == dependency {
            return true;
        }

        match (self, dependency) {
            #[cfg(all(feature = "lune", feature = "luau"))]
            (TargetKind::Lune, TargetKind::Luau) => true,

            #[cfg(feature = "roblox")]
            (TargetKind::RobloxServer, TargetKind::Roblox) => true,

            _ => false,
        }
    }

    /// The folder to store packages in for this target
    /// self is the project's target, dependency is the target of the dependency
    pub fn packages_folder(&self, dependency: &Self) -> String {
        if self == dependency {
            return "packages".to_string();
        }

        format!("{dependency}_packages")
    }
}

/// A target of a package
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case", tag = "environment")]
pub enum Target {
    /// A Roblox target
    #[cfg(feature = "roblox")]
    Roblox {
        /// The path to the lib export file
        #[serde(default)]
        lib: Option<RelativePathBuf>,
        /// The files to include in the sync tool's config
        #[serde(default)]
        build_files: BTreeSet<String>,
    },
    /// A Roblox server target
    #[cfg(feature = "roblox")]
    RobloxServer {
        /// The path to the lib export file
        #[serde(default)]
        lib: Option<RelativePathBuf>,
        /// The files to include in the sync tool's config
        #[serde(default)]
        build_files: BTreeSet<String>,
    },
    /// A Lune target
    #[cfg(feature = "lune")]
    Lune {
        /// The path to the lib export file
        #[serde(default)]
        lib: Option<RelativePathBuf>,
        /// The path to the bin export file
        #[serde(default)]
        bin: Option<RelativePathBuf>,
    },
    /// A Luau target
    #[cfg(feature = "luau")]
    Luau {
        /// The path to the lib export file
        #[serde(default)]
        lib: Option<RelativePathBuf>,
        /// The path to the bin export file
        #[serde(default)]
        bin: Option<RelativePathBuf>,
    },
}

impl Target {
    /// Returns the kind of this target
    pub fn kind(&self) -> TargetKind {
        match self {
            #[cfg(feature = "roblox")]
            Target::Roblox { .. } => TargetKind::Roblox,
            #[cfg(feature = "roblox")]
            Target::RobloxServer { .. } => TargetKind::RobloxServer,
            #[cfg(feature = "lune")]
            Target::Lune { .. } => TargetKind::Lune,
            #[cfg(feature = "luau")]
            Target::Luau { .. } => TargetKind::Luau,
        }
    }

    /// Returns the path to the lib export file
    pub fn lib_path(&self) -> Option<&RelativePathBuf> {
        match self {
            #[cfg(feature = "roblox")]
            Target::Roblox { lib, .. } => lib.as_ref(),
            #[cfg(feature = "roblox")]
            Target::RobloxServer { lib, .. } => lib.as_ref(),
            #[cfg(feature = "lune")]
            Target::Lune { lib, .. } => lib.as_ref(),
            #[cfg(feature = "luau")]
            Target::Luau { lib, .. } => lib.as_ref(),
        }
    }

    /// Returns the path to the bin export file
    pub fn bin_path(&self) -> Option<&RelativePathBuf> {
        match self {
            #[cfg(feature = "roblox")]
            Target::Roblox { .. } => None,
            #[cfg(feature = "roblox")]
            Target::RobloxServer { .. } => None,
            #[cfg(feature = "lune")]
            Target::Lune { bin, .. } => bin.as_ref(),
            #[cfg(feature = "luau")]
            Target::Luau { bin, .. } => bin.as_ref(),
        }
    }

    /// Returns the Roblox build files
    pub fn build_files(&self) -> Option<&BTreeSet<String>> {
        match self {
            #[cfg(feature = "roblox")]
            Target::Roblox { build_files, .. } => Some(build_files),
            #[cfg(feature = "roblox")]
            Target::RobloxServer { build_files, .. } => Some(build_files),
            _ => None,
        }
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind())
    }
}

#[cfg(feature = "roblox")]
/// The kind of a Roblox place property
#[derive(
    SerializeDisplay, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd,
)]
#[serde(rename_all = "snake_case")]
pub enum RobloxPlaceKind {
    /// The shared dependencies location
    Shared,
    /// The server dependencies location
    Server,
}

#[cfg(feature = "roblox")]
impl TryInto<RobloxPlaceKind> for &TargetKind {
    type Error = ();

    fn try_into(self) -> Result<RobloxPlaceKind, Self::Error> {
        match self {
            TargetKind::Roblox => Ok(RobloxPlaceKind::Shared),
            TargetKind::RobloxServer => Ok(RobloxPlaceKind::Server),
            _ => Err(()),
        }
    }
}

#[cfg(feature = "roblox")]
impl Display for RobloxPlaceKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RobloxPlaceKind::Shared => write!(f, "shared"),
            RobloxPlaceKind::Server => write!(f, "server"),
        }
    }
}

/// Errors that can occur when working with targets
pub mod errors {
    use thiserror::Error;

    /// Errors that can occur when parsing a target kind from a string
    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum TargetKindFromStr {
        /// The target kind is unknown
        #[error("unknown target kind {0}")]
        Unknown(String),
    }
}
