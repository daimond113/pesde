use std::{fmt::Display, str::FromStr};

use serde::{de::Visitor, Deserialize, Serialize};
use thiserror::Error;

/// A package name
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageName(String, String);

/// An error that occurred while validating a package name part (scope or name)
#[derive(Debug, Error)]
pub enum PackageNameValidationError {
    /// The package name part is empty
    #[error("package name part cannot be empty")]
    EmptyPart,
    /// The package name part contains invalid characters (only lowercase ASCII characters, numbers, and underscores are allowed)
    #[error("package name {0} part can only contain lowercase ASCII characters, numbers, and underscores")]
    InvalidPart(String),
    /// The package name part is too long (it cannot be longer than 24 characters)
    #[error("package name {0} part cannot be longer than 24 characters")]
    PartTooLong(String),
}

/// Validates a package name part (scope or name)
pub fn validate_part(part: &str) -> Result<(), PackageNameValidationError> {
    if part.is_empty() {
        return Err(PackageNameValidationError::EmptyPart);
    }

    if !part
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(PackageNameValidationError::InvalidPart(part.to_string()));
    }

    if part.len() > 24 {
        return Err(PackageNameValidationError::PartTooLong(part.to_string()));
    }

    Ok(())
}

const SEPARATOR: char = '/';
const ESCAPED_SEPARATOR: char = '-';

/// An error that occurred while parsing an escaped package name
#[derive(Debug, Error)]
pub enum EscapedPackageNameError {
    /// This is not a valid escaped package name
    #[error("package name is not in the format `scope{ESCAPED_SEPARATOR}name`")]
    Invalid,

    /// The package name is invalid
    #[error("invalid package name")]
    InvalidName(#[from] PackageNameValidationError),
}

impl PackageName {
    /// Creates a new package name
    pub fn new(scope: &str, name: &str) -> Result<Self, PackageNameValidationError> {
        validate_part(scope)?;
        validate_part(name)?;

        Ok(Self(scope.to_string(), name.to_string()))
    }

    /// Parses an escaped package name
    pub fn from_escaped(s: &str) -> Result<Self, EscapedPackageNameError> {
        let (scope, name) = s
            .split_once(ESCAPED_SEPARATOR)
            .ok_or(EscapedPackageNameError::Invalid)?;
        Ok(Self::new(scope, name)?)
    }

    /// Gets the scope of the package name
    pub fn scope(&self) -> &str {
        &self.0
    }

    /// Gets the name of the package name
    pub fn name(&self) -> &str {
        &self.1
    }

    /// Gets the escaped form (for use in file names, etc.) of the package name
    pub fn escaped(&self) -> String {
        format!("{}{ESCAPED_SEPARATOR}{}", self.0, self.1)
    }

    /// Gets the parts of the package name
    pub fn parts(&self) -> (&str, &str) {
        (&self.0, &self.1)
    }
}

impl Display for PackageName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{SEPARATOR}{}", self.0, self.1)
    }
}

/// An error that occurred while parsing a package name
#[derive(Debug, Error)]
pub enum FromStrPackageNameParseError {
    /// This is not a valid package name
    #[error("package name is not in the format `scope{SEPARATOR}name`")]
    Invalid,
    /// The package name is invalid
    #[error("invalid name part")]
    InvalidPart(#[from] PackageNameValidationError),
}

impl FromStr for PackageName {
    type Err = FromStrPackageNameParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(SEPARATOR).collect();
        if parts.len() != 2 {
            return Err(FromStrPackageNameParseError::Invalid);
        }

        Ok(PackageName::new(parts[0], parts[1])?)
    }
}

impl Serialize for PackageName {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

struct PackageNameVisitor;

impl<'de> Visitor<'de> for PackageNameVisitor {
    type Value = PackageName;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a string in the format `scope{SEPARATOR}name`")
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        v.parse().map_err(|e| E::custom(e))
    }
}

impl<'de> Deserialize<'de> for PackageName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<PackageName, D::Error> {
        deserializer.deserialize_str(PackageNameVisitor)
    }
}
