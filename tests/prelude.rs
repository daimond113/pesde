use std::{
    any::Any,
    collections::{BTreeSet, HashMap},
    sync::Arc,
};
use url::Url;

use pesde::{
    index::{
        ConfigError, CreatePackageVersionError, CredentialsFn, Index, IndexConfig, IndexFile,
        IndexFileEntry, IndexPackageError, ScopeOwners, ScopeOwnersError,
    },
    manifest::Manifest,
    package_name::PackageName,
};

/// An in-memory implementation of the [`Index`] trait. Used for testing.
#[derive(Debug, Clone)]
pub struct InMemoryIndex {
    packages: HashMap<String, (BTreeSet<u64>, IndexFile)>,
    url: Url,
}

impl Default for InMemoryIndex {
    fn default() -> Self {
        Self {
            packages: HashMap::new(),
            url: Url::parse("https://example.com").unwrap(),
        }
    }
}

impl InMemoryIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_scope(mut self, scope: &str, owners: BTreeSet<u64>) -> Self {
        self.packages
            .insert(scope.to_string(), (owners, IndexFile::default()));
        self
    }

    pub fn with_package(mut self, scope: &str, index_file: IndexFileEntry) -> Self {
        self.packages
            .entry(scope.to_string())
            .or_insert_with(|| (BTreeSet::new(), IndexFile::default()))
            .1
            .insert(index_file);
        self
    }
}

impl Index for InMemoryIndex {
    fn scope_owners(&self, scope: &str) -> Result<Option<ScopeOwners>, ScopeOwnersError> {
        Ok(self.packages.get(scope).map(|(owners, _)| owners).cloned())
    }

    fn create_scope_for(
        &mut self,
        scope: &str,
        owners: &ScopeOwners,
    ) -> Result<bool, ScopeOwnersError> {
        self.packages
            .insert(scope.to_string(), (owners.clone(), IndexFile::default()));
        Ok(true)
    }

    fn package(&self, name: &PackageName) -> Result<Option<IndexFile>, IndexPackageError> {
        Ok(self
            .packages
            .get(name.scope())
            .map(|(_, file)| file.clone()))
    }

    fn create_package_version(
        &mut self,
        manifest: &Manifest,
        uploader: &u64,
    ) -> Result<Option<IndexFileEntry>, CreatePackageVersionError> {
        let scope = manifest.name.scope();

        if let Some(owners) = self.scope_owners(scope)? {
            if !owners.contains(uploader) {
                return Err(CreatePackageVersionError::MissingScopeOwnership);
            }
        } else if !self.create_scope_for(scope, &BTreeSet::from([*uploader]))? {
            return Err(CreatePackageVersionError::MissingScopeOwnership);
        }

        let package = self.packages.get_mut(scope).unwrap();

        let entry: IndexFileEntry = manifest.clone().try_into()?;
        package.1.insert(entry.clone());

        Ok(Some(entry))
    }

    fn config(&self) -> Result<IndexConfig, ConfigError> {
        Ok(IndexConfig {
            download: None,
            api: "http://127.0.0.1:8080".parse().unwrap(),
            github_oauth_client_id: "".to_string(),
            custom_registry_allowed: false,
            git_allowed: false,
        })
    }

    fn credentials_fn(&self) -> Option<&Arc<CredentialsFn>> {
        None
    }

    fn url(&self) -> &Url {
        &self.url
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
