use std::{
    fmt::Debug,
    hash::Hash,
    {fmt::Display, str::FromStr},
};

use cfg_if::cfg_if;
use serde::{
    de::{IntoDeserializer, Visitor},
    Deserialize, Serialize,
};
use thiserror::Error;

/// A package name
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StandardPackageName(String, String);

/// An error that occurred while validating a package name part (scope or name)
#[derive(Debug, Error)]
pub enum StandardPackageNameValidationError {
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
pub fn validate_part(part: &str) -> Result<(), StandardPackageNameValidationError> {
    if part.is_empty() {
        return Err(StandardPackageNameValidationError::EmptyPart);
    }

    if !part
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(StandardPackageNameValidationError::InvalidPart(
            part.to_string(),
        ));
    }

    if part.len() > 24 {
        return Err(StandardPackageNameValidationError::PartTooLong(
            part.to_string(),
        ));
    }

    Ok(())
}

/// A wally package name
#[cfg(feature = "wally")]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WallyPackageName(String, String);

/// An error that occurred while validating a wally package name part (scope or name)
#[cfg(feature = "wally")]
#[derive(Debug, Error)]
pub enum WallyPackageNameValidationError {
    /// The package name part is empty
    #[error("wally package name part cannot be empty")]
    EmptyPart,
    /// The package name part contains invalid characters (only lowercase ASCII characters, numbers, and dashes are allowed)
    #[error("wally package name {0} part can only contain lowercase ASCII characters, numbers, and dashes")]
    InvalidPart(String),
    /// The package name part is too long (it cannot be longer than 64 characters)
    #[error("wally package name {0} part cannot be longer than 64 characters")]
    PartTooLong(String),
}

/// Validates a wally package name part (scope or name)
#[cfg(feature = "wally")]
pub fn validate_wally_part(part: &str) -> Result<(), WallyPackageNameValidationError> {
    if part.is_empty() {
        return Err(WallyPackageNameValidationError::EmptyPart);
    }

    if !part
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(WallyPackageNameValidationError::InvalidPart(
            part.to_string(),
        ));
    }

    if part.len() > 64 {
        return Err(WallyPackageNameValidationError::PartTooLong(
            part.to_string(),
        ));
    }

    Ok(())
}

/// An error that occurred while parsing an escaped package name
#[derive(Debug, Error)]
pub enum EscapedPackageNameError<E> {
    /// This package name is missing a prefix
    #[error("package name is missing prefix {0}")]
    MissingPrefix(String),

    /// This is not a valid escaped package name
    #[error("package name {0} is not in the format `scope{ESCAPED_SEPARATOR}name`")]
    Invalid(String),

    /// The package name is invalid
    #[error("invalid package name")]
    InvalidName(#[from] E),
}

/// An error that occurred while parsing a package name
#[derive(Debug, Error)]
pub enum FromStrPackageNameParseError<E> {
    /// This is not a valid package name
    #[error("package name {0} is not in the format `scope{SEPARATOR}name`")]
    Invalid(String),

    /// The package name is invalid
    #[error("invalid name part")]
    InvalidPart(#[from] E),
}

const SEPARATOR: char = '/';
const ESCAPED_SEPARATOR: char = '+';

macro_rules! name_impl {
    ($Name:ident, $Error:ident, $Visitor:ident, $validate:expr, $prefix:expr) => {
        impl $Name {
            /// Creates a new package name
            pub fn new(scope: &str, name: &str) -> Result<Self, $Error> {
                $validate(scope)?;
                $validate(name)?;

                Ok(Self(scope.to_string(), name.to_string()))
            }

            /// Parses an escaped package name
            pub fn from_escaped(s: &str) -> Result<Self, EscapedPackageNameError<$Error>> {
                if !s.starts_with($prefix) {
                    return Err(EscapedPackageNameError::MissingPrefix($prefix.to_string()));
                }

                let (scope, name) = &s[$prefix.len()..]
                    .split_once(ESCAPED_SEPARATOR)
                    .ok_or_else(|| EscapedPackageNameError::Invalid(s.to_string()))?;
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
                format!("{}{}{ESCAPED_SEPARATOR}{}", $prefix, self.0, self.1)
            }

            /// Gets the parts of the package name
            pub fn parts(&self) -> (&str, &str) {
                (&self.0, &self.1)
            }

            /// Returns the prefix for this package name
            pub fn prefix() -> &'static str {
                $prefix
            }
        }

        impl Display for $Name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}{}{SEPARATOR}{}", $prefix, self.0, self.1)
            }
        }

        impl FromStr for $Name {
            type Err = FromStrPackageNameParseError<$Error>;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let len = if s.starts_with($prefix) {
                    $prefix.len()
                } else {
                    0
                };

                let parts: Vec<&str> = s[len..].split(SEPARATOR).collect();
                if parts.len() != 2 {
                    return Err(FromStrPackageNameParseError::Invalid(s.to_string()));
                }

                Ok($Name::new(parts[0], parts[1])?)
            }
        }

        impl Serialize for $Name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Visitor<'de> for $Visitor {
            type Value = $Name;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    formatter,
                    "a string in the format `{}scope{SEPARATOR}name`",
                    $prefix
                )
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                v.parse().map_err(|e| E::custom(e))
            }
        }

        impl<'de> Deserialize<'de> for $Name {
            fn deserialize<D: serde::Deserializer<'de>>(
                deserializer: D,
            ) -> Result<$Name, D::Error> {
                deserializer.deserialize_str($Visitor)
            }
        }
    };
}

struct StandardPackageNameVisitor;
#[cfg(feature = "wally")]
struct WallyPackageNameVisitor;

/// A package name
#[derive(Serialize, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(untagged)]
pub enum PackageName {
    /// A standard package name
    Standard(StandardPackageName),
    /// A wally package name
    #[cfg(feature = "wally")]
    Wally(WallyPackageName),
}

impl PackageName {
    /// Gets the scope of the package name
    pub fn scope(&self) -> &str {
        match self {
            PackageName::Standard(name) => name.scope(),
            #[cfg(feature = "wally")]
            PackageName::Wally(name) => name.scope(),
        }
    }

    /// Gets the name of the package name
    pub fn name(&self) -> &str {
        match self {
            PackageName::Standard(name) => name.name(),
            #[cfg(feature = "wally")]
            PackageName::Wally(name) => name.name(),
        }
    }

    /// Gets the escaped form (for use in file names, etc.) of the package name
    pub fn escaped(&self) -> String {
        match self {
            PackageName::Standard(name) => name.escaped(),
            #[cfg(feature = "wally")]
            PackageName::Wally(name) => name.escaped(),
        }
    }

    /// Gets the parts of the package name
    pub fn parts(&self) -> (&str, &str) {
        match self {
            PackageName::Standard(name) => name.parts(),
            #[cfg(feature = "wally")]
            PackageName::Wally(name) => name.parts(),
        }
    }

    /// Returns the prefix for this package name
    pub fn prefix(&self) -> &'static str {
        match self {
            PackageName::Standard(_) => StandardPackageName::prefix(),
            #[cfg(feature = "wally")]
            PackageName::Wally(_) => WallyPackageName::prefix(),
        }
    }
}

impl Display for PackageName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageName::Standard(name) => write!(f, "{name}"),
            #[cfg(feature = "wally")]
            PackageName::Wally(name) => write!(f, "{name}"),
        }
    }
}

impl From<StandardPackageName> for PackageName {
    fn from(name: StandardPackageName) -> Self {
        PackageName::Standard(name)
    }
}

#[cfg(feature = "wally")]
impl From<WallyPackageName> for PackageName {
    fn from(name: WallyPackageName) -> Self {
        PackageName::Wally(name)
    }
}

name_impl!(
    StandardPackageName,
    StandardPackageNameValidationError,
    StandardPackageNameVisitor,
    validate_part,
    ""
);

#[cfg(feature = "wally")]
name_impl!(
    WallyPackageName,
    WallyPackageNameValidationError,
    WallyPackageNameVisitor,
    validate_wally_part,
    "wally#"
);

impl<'de> Deserialize<'de> for PackageName {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;

        cfg_if! {
            if #[cfg(feature = "wally")] {
                if s.starts_with(WallyPackageName::prefix()) {
                    return Ok(PackageName::Wally(
                        WallyPackageName::deserialize(s.into_deserializer())?,
                    ));
                }
            }
        }

        Ok(PackageName::Standard(StandardPackageName::deserialize(
            s.into_deserializer(),
        )?))
    }
}

/// An error that occurred while parsing a package name
#[derive(Debug, Error)]
pub enum FromStrPackageNameError {
    /// Error parsing the package name as a standard package name
    #[error("error parsing standard package name")]
    Standard(#[from] FromStrPackageNameParseError<StandardPackageNameValidationError>),

    /// Error parsing the package name as a wally package name
    #[cfg(feature = "wally")]
    #[error("error parsing wally package name")]
    Wally(#[from] FromStrPackageNameParseError<WallyPackageNameValidationError>),
}

impl FromStr for PackageName {
    type Err = FromStrPackageNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        cfg_if! {
            if #[cfg(feature = "wally")] {
                if s.starts_with(WallyPackageName::prefix()) {
                    return Ok(PackageName::Wally(WallyPackageName::from_str(s)?));
                }
            }
        }

        Ok(PackageName::Standard(StandardPackageName::from_str(s)?))
    }
}

/// An error that occurred while parsing an escaped package name
#[derive(Debug, Error)]
pub enum FromEscapedStrPackageNameError {
    /// Error parsing the package name as a standard package name
    #[error("error parsing standard package name")]
    Standard(#[from] EscapedPackageNameError<StandardPackageNameValidationError>),

    /// Error parsing the package name as a wally package name
    #[cfg(feature = "wally")]
    #[error("error parsing wally package name")]
    Wally(#[from] EscapedPackageNameError<WallyPackageNameValidationError>),
}

impl PackageName {
    /// Like `from_str`, but for escaped package names
    pub fn from_escaped_str(s: &str) -> Result<Self, FromEscapedStrPackageNameError> {
        cfg_if! {
            if #[cfg(feature = "wally")] {
                if s.starts_with(WallyPackageName::prefix()) {
                    return Ok(PackageName::Wally(WallyPackageName::from_escaped(s)?));
                }
            }
        }

        Ok(PackageName::Standard(StandardPackageName::from_escaped(s)?))
    }
}
