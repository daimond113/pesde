use relative_path::RelativePathBuf;
use semver::Version;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    str::FromStr,
};

use crate::{names::PackageName, source::DependencySpecifiers};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetKind {
    #[cfg(feature = "roblox")]
    Roblox,
    #[cfg(feature = "lune")]
    Lune,
    #[cfg(feature = "luau")]
    Luau,
}

impl Display for TargetKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "roblox")]
            TargetKind::Roblox => write!(f, "roblox"),
            #[cfg(feature = "lune")]
            TargetKind::Lune => write!(f, "lune"),
            #[cfg(feature = "luau")]
            TargetKind::Luau => write!(f, "luau"),
        }
    }
}

impl TargetKind {
    // self is the project's target, dependency is the target of the dependency
    pub fn is_compatible_with(&self, dependency: &Self) -> bool {
        if self == dependency {
            return true;
        }

        match (self, dependency) {
            #[cfg(all(feature = "lune", feature = "luau"))]
            (TargetKind::Lune, TargetKind::Luau) => true,

            _ => false,
        }
    }

    pub fn packages_folder(&self, dependency: &Self) -> String {
        if self == dependency {
            return "packages".to_string();
        }

        format!("{}_packages", dependency)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case", tag = "environment", remote = "Self")]
pub enum Target {
    #[cfg(feature = "roblox")]
    Roblox { lib: RelativePathBuf },
    #[cfg(feature = "lune")]
    Lune {
        lib: Option<RelativePathBuf>,
        bin: Option<RelativePathBuf>,
    },
    #[cfg(feature = "luau")]
    Luau {
        lib: Option<RelativePathBuf>,
        bin: Option<RelativePathBuf>,
    },
}

impl Target {
    pub fn kind(&self) -> TargetKind {
        match self {
            #[cfg(feature = "roblox")]
            Target::Roblox { .. } => TargetKind::Roblox,
            #[cfg(feature = "lune")]
            Target::Lune { .. } => TargetKind::Lune,
            #[cfg(feature = "luau")]
            Target::Luau { .. } => TargetKind::Luau,
        }
    }

    pub fn lib_path(&self) -> Option<&RelativePathBuf> {
        match self {
            #[cfg(feature = "roblox")]
            Target::Roblox { lib } => Some(lib),
            #[cfg(feature = "lune")]
            Target::Lune { lib, .. } => lib.as_ref(),
            #[cfg(feature = "luau")]
            Target::Luau { lib, .. } => lib.as_ref(),
        }
    }
}

impl Serialize for Target {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for Target {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let target = Self::deserialize(deserializer)?;

        match &target {
            #[cfg(feature = "lune")]
            Target::Lune { lib, bin } => {
                if lib.is_none() && bin.is_none() {
                    return Err(serde::de::Error::custom(
                        "one of `lib` or `bin` exports must be defined",
                    ));
                }
            }

            #[cfg(feature = "luau")]
            Target::Luau { lib, bin } => {
                if lib.is_none() && bin.is_none() {
                    return Err(serde::de::Error::custom(
                        "one of `lib` or `bin` exports must be defined",
                    ));
                }
            }

            #[allow(unreachable_patterns)]
            _ => {}
        };

        Ok(target)
    }
}

impl Display for Target {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.kind())
    }
}

#[derive(
    Debug, DeserializeFromStr, SerializeDisplay, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct OverrideKey(pub Vec<Vec<String>>);

impl FromStr for OverrideKey {
    type Err = errors::OverrideKeyFromStr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let overrides = s
            .split(',')
            .map(|overrides| overrides.split('>').map(|s| s.to_string()).collect())
            .collect::<Vec<Vec<String>>>();

        if overrides.is_empty() {
            return Err(errors::OverrideKeyFromStr::Empty);
        }

        Ok(Self(overrides))
    }
}

impl Display for OverrideKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|overrides| {
                    overrides
                        .iter()
                        .map(|o| o.as_str())
                        .collect::<Vec<_>>()
                        .join(">")
                })
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Manifest {
    pub name: PackageName,
    pub version: Version,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub authors: Option<Vec<String>>,
    #[serde(default)]
    pub repository: Option<String>,
    pub target: Target,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub scripts: BTreeMap<String, RelativePathBuf>,
    #[serde(default)]
    pub indices: BTreeMap<String, url::Url>,
    #[cfg(feature = "wally-compat")]
    #[serde(default)]
    pub wally_indices: BTreeMap<String, url::Url>,
    #[cfg(all(feature = "wally-compat", feature = "roblox"))]
    #[serde(default)]
    pub sourcemap_generator: Option<String>,
    #[serde(default)]
    pub overrides: BTreeMap<OverrideKey, DependencySpecifiers>,

    #[serde(default)]
    pub dependencies: BTreeMap<String, DependencySpecifiers>,
    #[serde(default)]
    pub peer_dependencies: BTreeMap<String, DependencySpecifiers>,
    #[serde(default)]
    pub dev_dependencies: BTreeMap<String, DependencySpecifiers>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    Standard,
    Dev,
    Peer,
}

impl Manifest {
    pub fn all_dependencies(
        &self,
    ) -> Result<
        BTreeMap<String, (DependencySpecifiers, DependencyType)>,
        errors::AllDependenciesError,
    > {
        let mut all_deps = BTreeMap::new();

        for (deps, ty) in [
            (&self.dependencies, DependencyType::Standard),
            (&self.peer_dependencies, DependencyType::Peer),
            (&self.dev_dependencies, DependencyType::Dev),
        ] {
            for (alias, spec) in deps {
                if all_deps.insert(alias.clone(), (spec.clone(), ty)).is_some() {
                    return Err(errors::AllDependenciesError::AliasConflict(alias.clone()));
                }
            }
        }

        Ok(all_deps)
    }
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum OverrideKeyFromStr {
        #[error("empty override key")]
        Empty,
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum AllDependenciesError {
        #[error("another specifier is already using the alias {0}")]
        AliasConflict(String),
    }
}
