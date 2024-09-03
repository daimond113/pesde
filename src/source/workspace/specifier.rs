use crate::{names::PackageName, source::DependencySpecifier};
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{fmt::Display, str::FromStr};

/// The specifier for a workspace dependency
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct WorkspaceDependencySpecifier {
    /// The name of the workspace package
    #[serde(rename = "workspace")]
    pub name: PackageName,
    /// The version type to use when publishing the package
    #[serde(default, rename = "version")]
    pub version_type: VersionType,
}
impl DependencySpecifier for WorkspaceDependencySpecifier {}

impl Display for WorkspaceDependencySpecifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "workspace:{}{}", self.version_type, self.name)
    }
}

/// The type of version to use when publishing a package
#[derive(
    Debug, SerializeDisplay, DeserializeFromStr, Clone, Copy, PartialEq, Eq, Hash, Default,
)]
pub enum VersionType {
    /// The "^" version type
    #[default]
    Caret,
    /// The "~" version type
    Tilde,
    /// The "=" version type
    Exact,
    /// The "*" version type
    Wildcard,
}

impl Display for VersionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionType::Caret => write!(f, "^"),
            VersionType::Tilde => write!(f, "~"),
            VersionType::Exact => write!(f, "="),
            VersionType::Wildcard => write!(f, "*"),
        }
    }
}

impl FromStr for VersionType {
    type Err = errors::VersionTypeFromStr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "^" => Ok(VersionType::Caret),
            "~" => Ok(VersionType::Tilde),
            "=" => Ok(VersionType::Exact),
            "*" => Ok(VersionType::Wildcard),
            _ => Err(errors::VersionTypeFromStr::InvalidVersionType(
                s.to_string(),
            )),
        }
    }
}

/// Errors that can occur when using a version type
pub mod errors {
    use thiserror::Error;

    /// Errors that can occur when parsing a version type
    #[derive(Debug, Error)]
    pub enum VersionTypeFromStr {
        /// The version type is invalid
        #[error("invalid version type: {0}")]
        InvalidVersionType(String),
    }
}
