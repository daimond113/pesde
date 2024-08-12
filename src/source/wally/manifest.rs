use std::collections::BTreeMap;

use semver::{Version, VersionReq};
use serde::{Deserialize, Deserializer};

use crate::{
    manifest::{errors, DependencyType},
    names::wally::WallyPackageName,
    source::{specifiers::DependencySpecifiers, wally::specifier::WallyDependencySpecifier},
};

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct WallyPackage {
    pub name: WallyPackageName,
    pub version: Version,
    pub registry: url::Url,
}

pub fn deserialize_specifiers<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, WallyDependencySpecifier>, D::Error> {
    // specifier is in form of `name@version_req`
    BTreeMap::<String, String>::deserialize(deserializer)?
        .into_iter()
        .map(|(k, v)| {
            let (name, version) = v.split_once('@').ok_or_else(|| {
                serde::de::Error::custom("invalid specifier format, expected `name@version_req`")
            })?;

            Ok((
                k,
                WallyDependencySpecifier {
                    name: name.parse().map_err(serde::de::Error::custom)?,
                    version: VersionReq::parse(version).map_err(serde::de::Error::custom)?,
                    index: None,
                },
            ))
        })
        .collect()
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct WallyManifest {
    pub package: WallyPackage,
    #[serde(default, deserialize_with = "deserialize_specifiers")]
    pub dependencies: BTreeMap<String, WallyDependencySpecifier>,
    #[serde(default, deserialize_with = "deserialize_specifiers")]
    pub server_dependencies: BTreeMap<String, WallyDependencySpecifier>,
    #[serde(default, deserialize_with = "deserialize_specifiers")]
    pub dev_dependencies: BTreeMap<String, WallyDependencySpecifier>,
}

impl WallyManifest {
    /// Get all dependencies from the manifest
    pub fn all_dependencies(
        &self,
    ) -> Result<
        BTreeMap<String, (DependencySpecifiers, DependencyType)>,
        errors::AllDependenciesError,
    > {
        let mut all_deps = BTreeMap::new();

        for (deps, ty) in [
            (&self.dependencies, DependencyType::Standard),
            (&self.server_dependencies, DependencyType::Standard),
            (&self.dev_dependencies, DependencyType::Dev),
        ] {
            for (alias, spec) in deps {
                let mut spec = spec.clone();
                spec.index = Some(self.package.registry.to_string());

                if all_deps
                    .insert(alias.clone(), (DependencySpecifiers::Wally(spec), ty))
                    .is_some()
                {
                    return Err(errors::AllDependenciesError::AliasConflict(alias.clone()));
                }
            }
        }

        Ok(all_deps)
    }
}
