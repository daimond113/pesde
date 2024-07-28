use crate::manifest::target::TargetKind;
use semver::Version;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{fmt::Display, str::FromStr};

#[derive(
    Debug, SerializeDisplay, DeserializeFromStr, Clone, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct VersionId(pub(crate) Version, pub(crate) TargetKind);

impl VersionId {
    pub fn new(version: Version, target: TargetKind) -> Self {
        VersionId(version, target)
    }

    pub fn version(&self) -> &Version {
        &self.0
    }

    pub fn target(&self) -> &TargetKind {
        &self.1
    }

    pub fn escaped(&self) -> String {
        format!("{}+{}", self.0, self.1)
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

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum VersionIdParseError {
        #[error("malformed entry key {0}")]
        Malformed(String),

        #[error("malformed version")]
        Version(#[from] semver::Error),

        #[error("malformed target")]
        Target(#[from] crate::manifest::target::errors::TargetKindFromStr),
    }
}
