use crate::source::{pesde, traits::DependencySpecifier};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// All possible dependency specifiers
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum DependencySpecifiers {
    /// A pesde dependency specifier
    Pesde(pesde::specifier::PesdeDependencySpecifier),
    /// A Wally dependency specifier
    #[cfg(feature = "wally-compat")]
    Wally(crate::source::wally::specifier::WallyDependencySpecifier),
    /// A Git dependency specifier
    Git(crate::source::git::specifier::GitDependencySpecifier),
}
impl DependencySpecifier for DependencySpecifiers {}

impl Display for DependencySpecifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencySpecifiers::Pesde(specifier) => write!(f, "{specifier}"),
            #[cfg(feature = "wally-compat")]
            DependencySpecifiers::Wally(specifier) => write!(f, "{specifier}"),
            DependencySpecifiers::Git(specifier) => write!(f, "{specifier}"),
        }
    }
}
