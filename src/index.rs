use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    fs::create_dir_all,
    hash::Hash,
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::{DateTime, Utc};
use git2::{build::RepoBuilder, Remote, Repository, Signature};
use log::debug;
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use crate::{
    dependencies::DependencySpecifier,
    manifest::{DependencyType, Manifest, Realm},
    package_name::PackageName,
};

/// Owners of a scope
pub type ScopeOwners = BTreeSet<u64>;

/// A packages index
pub trait Index: Send + Sync + Debug + Any + 'static {
    /// Gets the owners of a scope
    fn scope_owners(&self, scope: &str) -> Result<Option<ScopeOwners>, ScopeOwnersError>;

    /// Creates a scope
    fn create_scope_for(
        &mut self,
        scope: &str,
        owners: &ScopeOwners,
    ) -> Result<bool, ScopeOwnersError>;

    /// Gets a package from the index
    fn package(&self, name: &PackageName) -> Result<Option<IndexFile>, IndexPackageError>;

    /// Creates a package version
    fn create_package_version(
        &mut self,
        manifest: &Manifest,
        uploader: &u64,
    ) -> Result<Option<IndexFileEntry>, CreatePackageVersionError>;

    /// Gets the index's configuration
    fn config(&self) -> Result<IndexConfig, ConfigError>;

    /// Returns a function that gets the credentials for a git repository
    fn credentials_fn(&self) -> Option<&Arc<CredentialsFn>>;

    /// Returns the URL of the index's repository
    fn url(&self) -> &Url;

    /// Returns the token to this index's registry
    fn registry_auth_token(&self) -> Option<&str> {
        None
    }

    /// Updates the index
    fn refresh(&self) -> Result<(), RefreshError> {
        Ok(())
    }

    /// Returns this as Any
    fn as_any(&self) -> &dyn Any;
}

/// A function that gets the credentials for a git repository
pub type CredentialsFn = Box<
    dyn Fn() -> Box<
            dyn FnMut(&str, Option<&str>, git2::CredentialType) -> Result<git2::Cred, git2::Error>,
        > + Send
        + Sync,
>;

/// The packages index
#[derive(Clone)]
pub struct GitIndex {
    path: PathBuf,
    repo_url: Url,
    registry_auth_token: Option<String>,
    pub(crate) credentials_fn: Option<Arc<CredentialsFn>>,
}

impl Debug for GitIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitIndex")
            .field("path", &self.path)
            .field("repo_url", &self.repo_url)
            .finish()
    }
}

impl Hash for GitIndex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.path.hash(state);
        self.repo_url.hash(state);
    }
}

impl PartialEq for GitIndex {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.repo_url == other.repo_url
    }
}

impl Eq for GitIndex {}

/// An error that occurred while getting the index's refspec
#[derive(Debug, Error)]
pub enum GetRefSpecError {
    /// An error that occurred while interacting with git
    #[error("error interacting with git")]
    Git(#[from] git2::Error),

    /// The refspec for the upstream branch was not found
    #[error("refspec not found for upstream branch {0}")]
    RefSpecNotFound(String),

    /// The refspec is not utf-8
    #[error("refspec not utf-8")]
    RefSpecNotUtf8,

    /// The upstream branch was not found
    #[error("upstream branch not found")]
    UpstreamBranchNotFound,

    /// The upstream branch is not utf-8
    #[error("upstream branch not utf-8")]
    UpstreamBranchNotUtf8,
}

/// An error that occurred while refreshing the index
#[derive(Debug, Error)]
pub enum RefreshError {
    /// An error that occurred while interacting with git
    #[error("error interacting with git")]
    Git(#[from] git2::Error),

    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while getting the index's refspec
    #[error("error getting refspec")]
    GetRefSpec(#[from] GetRefSpecError),
}

/// An error that occurred while interacting with the scope owners
#[derive(Debug, Error)]
pub enum ScopeOwnersError {
    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while deserializing the scope owners
    #[error("error deserializing scope owners")]
    ScopeOwnersDeser(#[source] serde_yaml::Error),

    /// An error that occurred while committing and pushing to the index
    #[error("error committing and pushing to the index")]
    CommitAndPush(#[from] CommitAndPushError),
}

/// An error that occurred while committing and pushing to the index
#[derive(Debug, Error)]
pub enum CommitAndPushError {
    /// An error that occurred while interacting with git
    #[error("error interacting with git")]
    Git(#[from] git2::Error),

    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while getting the index's refspec
    #[error("error getting refspec")]
    GetRefSpec(#[from] GetRefSpecError),
}

/// An error that occurred while getting a package from the index
#[derive(Debug, Error)]
pub enum IndexPackageError {
    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while deserializing the index file
    #[error("error deserializing index file")]
    FileDeser(#[source] serde_yaml::Error),

    /// An unknown error occurred
    #[error("unknown error")]
    Other(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// An error that occurred while creating a package version
#[derive(Debug, Error)]
pub enum CreatePackageVersionError {
    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while getting a package from the index
    #[error("error getting a package from the index")]
    IndexPackage(#[from] IndexPackageError),

    /// An error that occurred while serializing the index file
    #[error("error serializing index file")]
    FileSer(#[source] serde_yaml::Error),

    /// An error that occurred while committing and pushing to the index
    #[error("error committing and pushing to the index")]
    CommitAndPush(#[from] CommitAndPushError),

    /// An error that occurred while interacting with the scope owners
    #[error("error interacting with the scope owners")]
    ScopeOwners(#[from] ScopeOwnersError),

    /// The scope is missing ownership
    #[error("missing scope ownership")]
    MissingScopeOwnership,

    /// An error that occurred while converting a manifest to an index file entry
    #[error("error converting manifest to index file entry")]
    FromManifestIndexFileEntry(#[from] FromManifestIndexFileEntry),
}

/// An error that occurred while getting the index's configuration
#[derive(Debug, Error)]
pub enum ConfigError {
    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred while deserializing the index config
    #[error("error deserializing index config")]
    ConfigDeser(#[source] serde_yaml::Error),

    /// The index does not have a config file
    #[error("index does not have a config file - this is an issue with the index, please contact the maintainer of the index")]
    MissingConfig,
}

fn get_refspec(
    repo: &Repository,
    remote: &mut Remote,
) -> Result<(String, String), GetRefSpecError> {
    let upstream_branch_buf = repo.branch_upstream_name(
        repo.head()?
            .name()
            .ok_or(GetRefSpecError::UpstreamBranchNotFound)?,
    )?;
    let upstream_branch = upstream_branch_buf
        .as_str()
        .ok_or(GetRefSpecError::UpstreamBranchNotUtf8)?;

    let refspec_buf = remote
        .refspecs()
        .find(|r| r.direction() == git2::Direction::Fetch && r.dst_matches(upstream_branch))
        .ok_or(GetRefSpecError::RefSpecNotFound(
            upstream_branch.to_string(),
        ))?
        .rtransform(upstream_branch)?;
    let refspec = refspec_buf
        .as_str()
        .ok_or(GetRefSpecError::RefSpecNotUtf8)?;

    Ok((refspec.to_string(), upstream_branch.to_string()))
}

macro_rules! remote_callbacks {
    ($index:expr) => {{
        #[allow(unused_imports)]
        use crate::index::Index;
        let mut remote_callbacks = git2::RemoteCallbacks::new();

        if let Some(credentials) = &$index.credentials_fn() {
            let credentials = std::sync::Arc::clone(credentials);

            remote_callbacks.credentials(move |a, b, c| credentials()(a, b, c));
        }

        remote_callbacks
    }};
}
pub(crate) use remote_callbacks;

impl GitIndex {
    /// Creates a new git index. The `refresh` method must be called before using the index, preferably immediately after creating it.
    pub fn new<P: AsRef<Path>>(
        path: P,
        repo_url: &Url,
        credentials: Option<CredentialsFn>,
        registry_auth_token: Option<String>,
    ) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            repo_url: repo_url.clone(),
            credentials_fn: credentials.map(Arc::new),
            registry_auth_token,
        }
    }

    /// Gets the path of the index
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Commits and pushes to the index
    pub fn commit_and_push(
        &self,
        message: &str,
        signature: &Signature,
    ) -> Result<(), CommitAndPushError> {
        let repo = Repository::open(&self.path)?;

        let mut index = repo.index()?;
        index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let oid = index.write_tree()?;
        let tree = repo.find_tree(oid)?;

        let parent_commit = repo.head()?.peel_to_commit()?;

        repo.commit(
            Some("HEAD"),
            signature,
            signature,
            message,
            &tree,
            &[&parent_commit],
        )?;

        let mut remote = repo.find_remote("origin")?;

        let (refspec, _) = get_refspec(&repo, &mut remote)?;

        remote.push(
            &[&refspec],
            Some(git2::PushOptions::new().remote_callbacks(remote_callbacks!(self))),
        )?;

        Ok(())
    }
}

macro_rules! refresh_git_based_index {
    ($index:expr) => {{
        let repo = if $index.path.exists() {
            Repository::open(&$index.path).ok()
        } else {
            None
        };

        if let Some(repo) = repo {
            let mut remote = repo.find_remote("origin")?;
            let (refspec, upstream_branch) = get_refspec(&repo, &mut remote)?;

            remote.fetch(
                &[&refspec],
                Some(git2::FetchOptions::new().remote_callbacks(remote_callbacks!($index))),
                None,
            )?;

            let commit = repo.find_reference(&upstream_branch)?.peel_to_commit()?;

            debug!(
                "refreshing index, fetching {refspec}#{} from origin",
                commit.id().to_string()
            );

            repo.reset(&commit.into_object(), git2::ResetType::Hard, None)?;

            Ok(())
        } else {
            debug!(
                "refreshing index - first time, cloning {} into {}",
                $index.repo_url,
                $index.path.display()
            );
            create_dir_all(&$index.path)?;

            let mut fetch_options = git2::FetchOptions::new();
            fetch_options.remote_callbacks(remote_callbacks!($index));

            RepoBuilder::new()
                .fetch_options(fetch_options)
                .clone(&$index.repo_url.to_string(), &$index.path)?;

            Ok(())
        }
    }};
}

impl Index for GitIndex {
    fn scope_owners(&self, scope: &str) -> Result<Option<ScopeOwners>, ScopeOwnersError> {
        let path = self.path.join(scope).join("owners.yaml");

        if !path.exists() {
            return Ok(None);
        }

        let contents = std::fs::read(&path)?;
        let owners: ScopeOwners =
            serde_yaml::from_slice(&contents).map_err(ScopeOwnersError::ScopeOwnersDeser)?;

        Ok(Some(owners))
    }

    fn create_scope_for(
        &mut self,
        scope: &str,
        owners: &ScopeOwners,
    ) -> Result<bool, ScopeOwnersError> {
        let path = self.path.join(scope);

        if path.exists() {
            return Ok(false);
        }

        create_dir_all(&path)?;

        serde_yaml::to_writer(std::fs::File::create(path.join("owners.yaml"))?, owners)
            .map_err(ScopeOwnersError::ScopeOwnersDeser)?;

        Ok(true)
    }

    fn package(&self, name: &PackageName) -> Result<Option<IndexFile>, IndexPackageError> {
        let path = self.path.join(name.scope()).join(name.name());

        if !path.exists() {
            return Ok(None);
        }

        let contents = std::fs::read(&path)?;
        let file: IndexFile =
            serde_yaml::from_slice(&contents).map_err(IndexPackageError::FileDeser)?;

        Ok(Some(file))
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

        let path = self.path.join(scope);

        let mut file =
            if let Some(file) = self.package(&PackageName::Standard(manifest.name.clone()))? {
                if file.iter().any(|e| e.version == manifest.version) {
                    return Ok(None);
                }
                file
            } else {
                BTreeSet::new()
            };

        let entry: IndexFileEntry = manifest.clone().try_into()?;
        file.insert(entry.clone());

        serde_yaml::to_writer(
            std::fs::File::create(path.join(manifest.name.name()))?,
            &file,
        )
        .map_err(CreatePackageVersionError::FileSer)?;

        Ok(Some(entry))
    }

    fn config(&self) -> Result<IndexConfig, ConfigError> {
        let path = self.path.join("config.yaml");

        if !path.exists() {
            return Err(ConfigError::MissingConfig);
        }

        let contents = std::fs::read(&path)?;
        let config: IndexConfig =
            serde_yaml::from_slice(&contents).map_err(ConfigError::ConfigDeser)?;

        Ok(config)
    }

    fn credentials_fn(&self) -> Option<&Arc<CredentialsFn>> {
        self.credentials_fn.as_ref()
    }

    fn url(&self) -> &Url {
        &self.repo_url
    }

    fn registry_auth_token(&self) -> Option<&str> {
        self.registry_auth_token.as_deref()
    }

    fn refresh(&self) -> Result<(), RefreshError> {
        refresh_git_based_index!(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The configuration of the index
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct IndexConfig {
    /// The URL of the index's API
    pub api: Url,
    /// The URL of the index's download API, defaults to `{API_URL}/v0/packages/{PACKAGE_AUTHOR}/{PACKAGE_NAME}/{PACKAGE_VERSION}`.
    /// Has the following variables:
    /// - `{API_URL}`: The URL of the index's API (without trailing `/`)
    /// - `{PACKAGE_AUTHOR}`: The author of the package
    /// - `{PACKAGE_NAME}`: The name of the package
    /// - `{PACKAGE_VERSION}`: The version of the package
    pub download: Option<String>,
    /// Whether to allow git dependencies
    #[serde(default)]
    pub git_allowed: bool,
    /// Whether to allow custom registries
    #[serde(default)]
    pub custom_registry_allowed: bool,
    /// The OAuth client ID for GitHub OAuth
    pub github_oauth_client_id: String,
}

impl IndexConfig {
    /// Gets the URL of the index's API
    pub fn api(&self) -> &str {
        self.api.as_str().trim_end_matches('/')
    }

    /// Gets the URL of the index's download API
    pub fn download(&self) -> String {
        self.download
            .as_ref()
            .unwrap_or(
                &"{API_URL}/v0/packages/{PACKAGE_AUTHOR}/{PACKAGE_NAME}/{PACKAGE_VERSION}"
                    .to_string(),
            )
            .replace("{API_URL}", self.api())
    }
}

/// An entry in the index file
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct IndexFileEntry {
    /// The version of the package
    pub version: Version,
    /// The realm of the package
    pub realm: Option<Realm>,
    /// When the package was published
    #[serde(default = "Utc::now")]
    pub published_at: DateTime<Utc>,

    /// A description of the package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The dependencies of the package
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, (DependencySpecifier, DependencyType)>,
}

/// An error that occurred while converting a manifest to an index file entry
#[derive(Debug, Error)]
pub enum FromManifestIndexFileEntry {
    /// An error that occurred because an index is not specified
    #[error("index {0} is not specified")]
    IndexNotSpecified(String),
}

impl TryFrom<Manifest> for IndexFileEntry {
    type Error = FromManifestIndexFileEntry;

    fn try_from(manifest: Manifest) -> Result<Self, Self::Error> {
        let dependencies = manifest.dependencies();
        let indices = manifest.indices;

        Ok(Self {
            version: manifest.version,
            realm: manifest.realm,
            published_at: Utc::now(),

            description: manifest.description,

            dependencies: dependencies
                .into_iter()
                .map(|(desired_name, (dep, ty))| {
                    Ok((
                        desired_name,
                        match dep {
                            DependencySpecifier::Registry(mut registry) => {
                                registry.index = indices
                                    .get(&registry.index)
                                    .ok_or_else(|| {
                                        FromManifestIndexFileEntry::IndexNotSpecified(
                                            registry.index.clone(),
                                        )
                                    })?
                                    .clone();
                                (DependencySpecifier::Registry(registry), ty)
                            }
                            d => (d, ty),
                        },
                    ))
                })
                .collect::<Result<_, _>>()?,
        })
    }
}

impl PartialOrd for IndexFileEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.version.cmp(&other.version))
    }
}

impl Ord for IndexFileEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.version.cmp(&other.version)
    }
}

/// An index file
pub type IndexFile = BTreeSet<IndexFileEntry>;

#[cfg(feature = "wally")]
#[derive(Clone)]
pub(crate) struct WallyIndex {
    repo_url: Url,
    registry_auth_token: Option<String>,
    credentials_fn: Option<Arc<CredentialsFn>>,
    pub(crate) path: PathBuf,
}

#[cfg(feature = "wally")]
impl Debug for WallyIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WallyIndex")
            .field("path", &self.path)
            .field("repo_url", &self.repo_url)
            .finish()
    }
}

#[cfg(feature = "wally")]
impl WallyIndex {
    pub(crate) fn new(
        repo_url: Url,
        registry_auth_token: Option<String>,
        path: &Path,
        credentials_fn: Option<Arc<CredentialsFn>>,
    ) -> Self {
        Self {
            repo_url,
            registry_auth_token,
            path: path.to_path_buf(),
            credentials_fn,
        }
    }
}

#[cfg(feature = "wally")]
impl Index for WallyIndex {
    fn scope_owners(&self, _scope: &str) -> Result<Option<ScopeOwners>, ScopeOwnersError> {
        unimplemented!("wally index is a virtual index meant for wally compatibility only")
    }

    fn create_scope_for(
        &mut self,
        _scope: &str,
        _owners: &ScopeOwners,
    ) -> Result<bool, ScopeOwnersError> {
        unimplemented!("wally index is a virtual index meant for wally compatibility only")
    }

    fn package(&self, name: &PackageName) -> Result<Option<IndexFile>, IndexPackageError> {
        let path = self.path.join(name.scope()).join(name.name());

        if !path.exists() {
            return Ok(None);
        }

        let file = std::fs::File::open(&path)?;
        let file = std::io::BufReader::new(file);

        let manifest_stream = serde_json::Deserializer::from_reader(file)
            .into_iter::<crate::dependencies::wally::WallyManifest>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| IndexPackageError::Other(Box::new(e)))?;

        Ok(Some(
            manifest_stream
                .into_iter()
                .map(|m| m.try_into())
                .collect::<Result<BTreeSet<_>, _>>()
                .map_err(|e| IndexPackageError::Other(Box::new(e)))?,
        ))
    }

    fn create_package_version(
        &mut self,
        _manifest: &Manifest,
        _uploader: &u64,
    ) -> Result<Option<IndexFileEntry>, CreatePackageVersionError> {
        unimplemented!("wally index is a virtual index meant for wally compatibility only")
    }

    fn config(&self) -> Result<IndexConfig, ConfigError> {
        unimplemented!("wally index is a virtual index meant for wally compatibility only")
    }

    fn credentials_fn(&self) -> Option<&Arc<CredentialsFn>> {
        self.credentials_fn.as_ref()
    }

    fn url(&self) -> &Url {
        &self.repo_url
    }

    fn registry_auth_token(&self) -> Option<&str> {
        self.registry_auth_token.as_deref()
    }

    fn refresh(&self) -> Result<(), RefreshError> {
        refresh_git_based_index!(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
