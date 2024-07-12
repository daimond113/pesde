use crate::{names::PackageName, source::DependencySpecifiers};
use relative_path::RelativePathBuf;
use semver::Version;
use serde::{de::Visitor, Deserialize, Deserializer, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    str::FromStr,
};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct Exports {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lib: Option<RelativePathBuf>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<RelativePathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    #[cfg(feature = "roblox")]
    Roblox,
    #[cfg(feature = "lune")]
    Lune,
    #[cfg(feature = "luau")]
    Luau,
}

impl Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "roblox")]
            Target::Roblox => write!(f, "roblox"),
            #[cfg(feature = "lune")]
            Target::Lune => write!(f, "lune"),
            #[cfg(feature = "luau")]
            Target::Luau => write!(f, "luau"),
        }
    }
}

impl Target {
    // self is the project's target, dependency is the target of the dependency
    fn is_compatible_with(&self, dependency: &Self) -> bool {
        if self == dependency {
            return true;
        }

        match (self, dependency) {
            #[cfg(all(feature = "lune", feature = "luau"))]
            (Target::Lune, Target::Luau) => true,

            _ => false,
        }
    }
}

#[derive(
    Debug, DeserializeFromStr, SerializeDisplay, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct OverrideKey(pub Vec<Vec<String>>);

impl FromStr for OverrideKey {
    type Err = errors::OverrideKeyFromStr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            s.split(',')
                .map(|overrides| overrides.split('>').map(|s| s.to_string()).collect())
                .collect(),
        ))
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

fn deserialize_dep_specs<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, DependencySpecifiers>, D::Error>
where
    D: Deserializer<'de>,
{
    struct SpecsVisitor;

    impl<'de> Visitor<'de> for SpecsVisitor {
        type Value = BTreeMap<String, DependencySpecifiers>;

        fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
            formatter.write_str("a map of dependency specifiers")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut specs = BTreeMap::new();

            while let Some((key, mut value)) = map.next_entry::<String, DependencySpecifiers>()? {
                value.set_alias(key.to_string());
                specs.insert(key, value);
            }

            Ok(specs)
        }
    }

    deserializer.deserialize_map(SpecsVisitor)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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
    #[serde(default)]
    pub exports: Exports,
    pub target: Target,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub indices: BTreeMap<String, url::Url>,
    #[cfg(feature = "wally")]
    #[serde(default)]
    pub wally_indices: BTreeMap<String, url::Url>,
    #[cfg(feature = "wally")]
    #[serde(default)]
    pub sourcemap_generator: Option<String>,
    #[serde(default)]
    pub overrides: BTreeMap<OverrideKey, DependencySpecifiers>,

    #[serde(default, deserialize_with = "deserialize_dep_specs")]
    pub dependencies: BTreeMap<String, DependencySpecifiers>,
    #[serde(default, deserialize_with = "deserialize_dep_specs")]
    pub peer_dependencies: BTreeMap<String, DependencySpecifiers>,
    #[serde(default, deserialize_with = "deserialize_dep_specs")]
    pub dev_dependencies: BTreeMap<String, DependencySpecifiers>,
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum OverrideKeyFromStr {}
}
