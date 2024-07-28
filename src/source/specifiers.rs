use crate::source::{pesde, traits::DependencySpecifier};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum DependencySpecifiers {
    Pesde(pesde::specifier::PesdeDependencySpecifier),
}
impl DependencySpecifier for DependencySpecifiers {}

impl Display for DependencySpecifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DependencySpecifiers::Pesde(specifier) => write!(f, "{specifier}"),
        }
    }
}
