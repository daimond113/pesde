use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

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

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum OverrideKeyFromStr {
        #[error("empty override key")]
        Empty,
    }
}
