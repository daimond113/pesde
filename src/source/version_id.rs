use crate::manifest::target::TargetKind;
use semver::Version;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{fmt::Display, str::FromStr};

/// A version ID, which is a combination of a version and a target
#[derive(
    Debug, SerializeDisplay, DeserializeFromStr, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct VersionId(pub(crate) Version, pub(crate) TargetKind);

impl VersionId {
    /// Creates a new version ID
    pub fn new(version: Version, target: TargetKind) -> Self {
        VersionId(version, target)
    }

    /// Access the version
    pub fn version(&self) -> &Version {
        &self.0
    }

    /// Access the target
    pub fn target(&self) -> &TargetKind {
        &self.1
    }

    /// Returns this version ID as a string that can be used in the filesystem
    pub fn escaped(&self) -> String {
        format!("{}+{}", self.0, self.1)
    }

    /// The reverse of `escaped`
    pub fn from_escaped(s: &str) -> Result<Self, errors::VersionIdParseError> {
        VersionId::from_str(s.replacen('+', " ", 1).as_str())
    }
}

impl Display for VersionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.0, self.1)
    }
}

impl FromStr for VersionId {
    type Err = errors::VersionIdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((version, target)) = s.split_once(' ') else {
            return Err(errors::VersionIdParseError::Malformed(s.to_string()));
        };

        let version = version.parse()?;
        let target = target.parse()?;

        Ok(VersionId(version, target))
    }
}

/// Errors that can occur when using a version ID
pub mod errors {
    use thiserror::Error;

    /// Errors that can occur when parsing a version ID
    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum VersionIdParseError {
        /// The version ID is malformed
        #[error("malformed version id {0}")]
        Malformed(String),

        /// The version is malformed
        #[error("malformed version")]
        Version(#[from] semver::Error),

        /// The target is malformed
        #[error("malformed target")]
        Target(#[from] crate::manifest::target::errors::TargetKindFromStr),
    }
}
