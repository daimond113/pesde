use gix::remote::Direction;
use pkg_ref::PesdePackageRef;
use relative_path::RelativePathBuf;
use reqwest::header::ACCEPT;
use serde::{Deserialize, Serialize};
use specifier::PesdeDependencySpecifier;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    hash::Hash,
    io::Read,
};

use crate::{
    manifest::{
        target::{Target, TargetKind},
        DependencyType,
    },
    names::{PackageName, PackageNames},
    source::{
        fs::{store_in_cas, FSEntry, PackageFS},
        DependencySpecifiers, PackageSource, ResolveResult, VersionId,
    },
    util::{authenticate_conn, hash},
    Project,
};

pub mod pkg_ref;
pub mod specifier;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct PesdePackageSource {
    repo_url: gix::Url,
}

pub const SCOPE_INFO_FILE: &str = "scope.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeInfo {
    pub owners: BTreeSet<u64>,
}

impl PesdePackageSource {
    pub fn new(repo_url: gix::Url) -> Self {
        Self { repo_url }
    }

    fn as_bytes(&self) -> Vec<u8> {
        self.repo_url.to_bstring().to_vec()
    }

    pub fn path(&self, project: &Project) -> std::path::PathBuf {
        project.data_dir.join("indices").join(hash(self.as_bytes()))
    }

    pub fn repo_url(&self) -> &gix::Url {
        &self.repo_url
    }

    pub(crate) fn tree<'a>(
        &'a self,
        repo: &'a gix::Repository,
    ) -> Result<gix::Tree, errors::TreeError> {
        // this is a bare repo, so this is the actual path
        let path = repo.path().to_path_buf();

        let remote = match repo.find_default_remote(Direction::Fetch) {
            Some(Ok(remote)) => remote,
            Some(Err(e)) => return Err(errors::TreeError::GetDefaultRemote(path, Box::new(e))),
            None => {
                return Err(errors::TreeError::NoDefaultRemote(path));
            }
        };

        let refspec = match remote.refspecs(Direction::Fetch).first() {
            Some(head) => head,
            None => return Err(errors::TreeError::NoRefSpecs(path)),
        };

        let spec_ref = refspec.to_ref();
        let local_ref = match spec_ref.local() {
            Some(local) => local
                .to_string()
                .replace('*', repo.branch_names().first().unwrap_or(&"main")),
            None => return Err(errors::TreeError::NoLocalRefSpec(path)),
        };

        let reference = match repo.find_reference(&local_ref) {
            Ok(reference) => reference,
            Err(e) => return Err(errors::TreeError::NoReference(local_ref.to_string(), e)),
        };

        let reference_name = reference.name().as_bstr().to_string();
        let id = match reference.into_fully_peeled_id() {
            Ok(id) => id,
            Err(e) => return Err(errors::TreeError::CannotPeel(reference_name, e)),
        };

        let id_str = id.to_string();
        let object = match id.object() {
            Ok(object) => object,
            Err(e) => return Err(errors::TreeError::CannotConvertToObject(id_str, e)),
        };

        match object.peel_to_tree() {
            Ok(tree) => Ok(tree),
            Err(e) => Err(errors::TreeError::CannotPeelToTree(id_str, e)),
        }
    }

    pub fn read_file<
        I: IntoIterator<Item = P> + Clone,
        P: ToString + PartialEq<gix::bstr::BStr>,
    >(
        &self,
        file_path: I,
        project: &Project,
    ) -> Result<Option<String>, errors::ReadFile> {
        let path = self.path(project);

        let repo = match gix::open(&path) {
            Ok(repo) => repo,
            Err(e) => return Err(errors::ReadFile::Open(path, Box::new(e))),
        };

        let tree = match self.tree(&repo) {
            Ok(tree) => tree,
            Err(e) => return Err(errors::ReadFile::Tree(path, Box::new(e))),
        };

        let file_path_str = file_path
            .clone()
            .into_iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(std::path::MAIN_SEPARATOR_STR);

        let mut lookup_buf = vec![];
        let entry = match tree.lookup_entry(file_path, &mut lookup_buf) {
            Ok(Some(entry)) => entry,
            Ok(None) => return Ok(None),
            Err(e) => return Err(errors::ReadFile::Lookup(file_path_str, e)),
        };

        let object = match entry.object() {
            Ok(object) => object,
            Err(e) => return Err(errors::ReadFile::Lookup(file_path_str, e)),
        };

        let blob = object.into_blob();
        let string = String::from_utf8(blob.data.clone())
            .map_err(|e| errors::ReadFile::Utf8(file_path_str, e))?;

        Ok(Some(string))
    }

    pub fn config(&self, project: &Project) -> Result<IndexConfig, errors::ConfigError> {
        let file = self.read_file(["config.toml"], project).map_err(Box::new)?;

        let string = match file {
            Some(s) => s,
            None => {
                return Err(errors::ConfigError::Missing(Box::new(
                    self.repo_url.clone(),
                )))
            }
        };

        let config: IndexConfig = toml::from_str(&string)?;

        Ok(config)
    }

    pub fn all_packages(
        &self,
        project: &Project,
    ) -> Result<BTreeMap<PackageName, IndexFile>, errors::AllPackagesError> {
        let path = self.path(project);

        let repo = match gix::open(&path) {
            Ok(repo) => repo,
            Err(e) => return Err(errors::AllPackagesError::Open(path, Box::new(e))),
        };

        let tree = match self.tree(&repo) {
            Ok(tree) => tree,
            Err(e) => return Err(errors::AllPackagesError::Tree(path, Box::new(e))),
        };

        let mut packages = BTreeMap::<PackageName, IndexFile>::new();

        for entry in tree.iter() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => return Err(errors::AllPackagesError::Decode(path, e)),
            };

            let object = match entry.object() {
                Ok(object) => object,
                Err(e) => return Err(errors::AllPackagesError::Convert(path, e)),
            };

            // directories will be trees, and files will be blobs
            if !matches!(object.kind, gix::object::Kind::Tree) {
                continue;
            }

            let package_scope = entry.filename().to_string();

            for inner_entry in object.into_tree().iter() {
                let inner_entry = match inner_entry {
                    Ok(entry) => entry,
                    Err(e) => return Err(errors::AllPackagesError::Decode(path, e)),
                };

                let object = match inner_entry.object() {
                    Ok(object) => object,
                    Err(e) => return Err(errors::AllPackagesError::Convert(path, e)),
                };

                if !matches!(object.kind, gix::object::Kind::Blob) {
                    continue;
                }

                let package_name = inner_entry.filename().to_string();

                if package_name == SCOPE_INFO_FILE {
                    continue;
                }

                let blob = object.into_blob();
                let string = String::from_utf8(blob.data.clone())
                    .map_err(|e| errors::AllPackagesError::Utf8(package_name.to_string(), e))?;

                let file: IndexFile = match toml::from_str(&string) {
                    Ok(file) => file,
                    Err(e) => {
                        return Err(errors::AllPackagesError::Deserialize(
                            package_name,
                            path,
                            Box::new(e),
                        ))
                    }
                };

                // if this panics, it's an issue with the index.
                let name = format!("{package_scope}/{package_name}").parse().unwrap();

                packages.insert(name, file);
            }
        }

        Ok(packages)
    }

    #[cfg(feature = "git2")]
    pub fn repo_git2(&self, project: &Project) -> Result<git2::Repository, git2::Error> {
        let path = self.path(project);

        git2::Repository::open_bare(&path)
    }
}

impl PackageSource for PesdePackageSource {
    type Ref = PesdePackageRef;
    type Specifier = PesdeDependencySpecifier;
    type RefreshError = errors::RefreshError;
    type ResolveError = errors::ResolveError;
    type DownloadError = errors::DownloadError;

    fn refresh(&self, project: &Project) -> Result<(), Self::RefreshError> {
        log::debug!("refreshing pesde index at {}", self.repo_url);

        let path = self.path(project);
        if path.exists() {
            let repo = match gix::open(&path) {
                Ok(repo) => repo,
                Err(e) => return Err(Self::RefreshError::Open(path, e)),
            };
            let remote = match repo.find_default_remote(Direction::Fetch) {
                Some(Ok(remote)) => remote,
                Some(Err(e)) => return Err(Self::RefreshError::GetDefaultRemote(path, e)),
                None => {
                    return Err(Self::RefreshError::NoDefaultRemote(path));
                }
            };

            let mut connection = remote
                .connect(Direction::Fetch)
                .map_err(|e| Self::RefreshError::Connect(self.repo_url.clone(), e))?;

            authenticate_conn(&mut connection, &project.auth_config);

            connection
                .prepare_fetch(gix::progress::Discard, Default::default())
                .map_err(|e| Self::RefreshError::PrepareFetch(self.repo_url.clone(), e))?
                .receive(gix::progress::Discard, &false.into())
                .map_err(|e| Self::RefreshError::Read(self.repo_url.clone(), e))?;

            return Ok(());
        }

        std::fs::create_dir_all(&path)?;

        let auth_config = project.auth_config.clone();

        gix::prepare_clone_bare(self.repo_url.clone(), &path)
            .map_err(|e| Self::RefreshError::Clone(self.repo_url.clone(), e))?
            .configure_connection(move |c| {
                authenticate_conn(c, &auth_config);
                Ok(())
            })
            .fetch_only(gix::progress::Discard, &false.into())
            .map_err(|e| Self::RefreshError::Fetch(self.repo_url.clone(), e))?;

        Ok(())
    }

    fn resolve(
        &self,
        specifier: &Self::Specifier,
        project: &Project,
        project_target: TargetKind,
    ) -> Result<ResolveResult<Self::Ref>, Self::ResolveError> {
        let (scope, name) = specifier.name.as_str();
        let string = match self.read_file([scope, name], project) {
            Ok(Some(s)) => s,
            Ok(None) => return Err(Self::ResolveError::NotFound(specifier.name.to_string())),
            Err(e) => {
                return Err(Self::ResolveError::Read(
                    specifier.name.to_string(),
                    Box::new(e),
                ))
            }
        };

        let entries: IndexFile = toml::from_str(&string)
            .map_err(|e| Self::ResolveError::Parse(specifier.name.to_string(), e))?;

        log::debug!("{} has {} possible entries", specifier.name, entries.len());

        Ok((
            PackageNames::Pesde(specifier.name.clone()),
            entries
                .into_iter()
                .filter(|(VersionId(version, target), _)| {
                    specifier.version.matches(version)
                        && specifier
                            .target
                            .map_or(project_target.is_compatible_with(target), |t| t == *target)
                })
                .map(|(id, entry)| {
                    let version = id.version().clone();

                    (
                        id,
                        PesdePackageRef {
                            name: specifier.name.clone(),
                            version,
                            index_url: self.repo_url.clone(),
                            dependencies: entry.dependencies,
                            target: entry.target,
                        },
                    )
                })
                .collect(),
        ))
    }

    fn download(
        &self,
        pkg_ref: &Self::Ref,
        project: &Project,
        reqwest: &reqwest::blocking::Client,
    ) -> Result<(PackageFS, Target), Self::DownloadError> {
        let config = self.config(project).map_err(Box::new)?;
        let index_file = project
            .cas_dir
            .join("index")
            .join(pkg_ref.name.escaped())
            .join(pkg_ref.version.to_string())
            .join(pkg_ref.target.to_string());

        match std::fs::read_to_string(&index_file) {
            Ok(s) => {
                log::debug!(
                    "using cached index file for package {}@{} {}",
                    pkg_ref.name,
                    pkg_ref.version,
                    pkg_ref.target
                );
                return Ok((toml::from_str::<PackageFS>(&s)?, pkg_ref.target.clone()));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(errors::DownloadError::ReadIndex(e)),
        }

        let url = config
            .download()
            .replace("{PACKAGE}", &pkg_ref.name.to_string().replace("/", "%2F"))
            .replace("{PACKAGE_VERSION}", &pkg_ref.version.to_string())
            .replace("{PACKAGE_TARGET}", &pkg_ref.target.to_string());

        let mut response = reqwest.get(url).header(ACCEPT, "application/octet-stream");

        if let Some(token) = &project.auth_config.pesde_token {
            log::debug!("using token for pesde package download");
            response = response.header("Authorization", format!("Bearer {token}"));
        }

        let response = response.send()?.error_for_status()?;
        let bytes = response.bytes()?;

        let mut decoder = flate2::read::GzDecoder::new(bytes.as_ref());
        let mut archive = tar::Archive::new(&mut decoder);

        let mut entries = BTreeMap::new();

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = RelativePathBuf::from_path(entry.path()?).unwrap();

            if entry.header().entry_type().is_dir() {
                entries.insert(path, FSEntry::Directory);

                continue;
            }

            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;

            let hash = store_in_cas(&project.cas_dir, &contents)?.0;
            entries.insert(path, FSEntry::File(hash));
        }

        let fs = PackageFS(entries);

        if let Some(parent) = index_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&index_file, toml::to_string(&fs)?)
            .map_err(errors::DownloadError::WriteIndex)?;

        Ok((fs, pkg_ref.target.clone()))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct IndexConfig {
    pub api: url::Url,
    pub download: Option<String>,
    #[serde(default)]
    pub git_allowed: bool,
    #[serde(default)]
    pub other_registries_allowed: bool,
    pub github_oauth_client_id: String,
}

impl IndexConfig {
    pub fn api(&self) -> &str {
        self.api.as_str().trim_end_matches('/')
    }

    pub fn download(&self) -> String {
        self.download
            .as_deref()
            .unwrap_or("{API_URL}/v0/packages/{PACKAGE}/{PACKAGE_VERSION}/{PACKAGE_TARGET}")
            .replace("{API_URL}", self.api())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct IndexFileEntry {
    pub target: Target,
    #[serde(default = "chrono::Utc::now")]
    pub published_at: chrono::DateTime<chrono::Utc>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, (DependencySpecifiers, DependencyType)>,
}

pub type IndexFile = BTreeMap<VersionId, IndexFileEntry>;

pub mod errors {
    use std::path::PathBuf;

    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum RefreshError {
        #[error("error interacting with the filesystem")]
        Io(#[from] std::io::Error),

        #[error("error opening repository at {0}")]
        Open(PathBuf, #[source] gix::open::Error),

        #[error("no default remote found in repository at {0}")]
        NoDefaultRemote(PathBuf),

        #[error("error getting default remote from repository at {0}")]
        GetDefaultRemote(PathBuf, #[source] gix::remote::find::existing::Error),

        #[error("error connecting to remote repository at {0}")]
        Connect(gix::Url, #[source] gix::remote::connect::Error),

        #[error("error preparing fetch from remote repository at {0}")]
        PrepareFetch(gix::Url, #[source] gix::remote::fetch::prepare::Error),

        #[error("error reading from remote repository at {0}")]
        Read(gix::Url, #[source] gix::remote::fetch::Error),

        #[error("error cloning repository from {0}")]
        Clone(gix::Url, #[source] gix::clone::Error),

        #[error("error fetching repository from {0}")]
        Fetch(gix::Url, #[source] gix::clone::fetch::Error),
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum TreeError {
        #[error("error interacting with the filesystem")]
        Io(#[from] std::io::Error),

        #[error("no default remote found in repository at {0}")]
        NoDefaultRemote(PathBuf),

        #[error("error getting default remote from repository at {0}")]
        GetDefaultRemote(PathBuf, #[source] Box<gix::remote::find::existing::Error>),

        #[error("no refspecs found in repository at {0}")]
        NoRefSpecs(PathBuf),

        #[error("no local refspec found in repository at {0}")]
        NoLocalRefSpec(PathBuf),

        #[error("no reference found for local refspec {0}")]
        NoReference(String, #[source] gix::reference::find::existing::Error),

        #[error("cannot peel reference {0}")]
        CannotPeel(String, #[source] gix::reference::peel::Error),

        #[error("error converting id {0} to object")]
        CannotConvertToObject(String, #[source] gix::object::find::existing::Error),

        #[error("error peeling object {0} to tree")]
        CannotPeelToTree(String, #[source] gix::object::peel::to_kind::Error),
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum ReadFile {
        #[error("error opening repository at {0}")]
        Open(PathBuf, #[source] Box<gix::open::Error>),

        #[error("error getting tree from repository at {0}")]
        Tree(PathBuf, #[source] Box<TreeError>),

        #[error("error looking up entry {0} in tree")]
        Lookup(String, #[source] gix::object::find::existing::Error),

        #[error("error parsing file for {0} as utf8")]
        Utf8(String, #[source] std::string::FromUtf8Error),
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum ResolveError {
        #[error("error interacting with the filesystem")]
        Io(#[from] std::io::Error),

        #[error("package {0} not found")]
        NotFound(String),

        #[error("error reading file for {0}")]
        Read(String, #[source] Box<ReadFile>),

        #[error("error parsing file for {0}")]
        Parse(String, #[source] toml::de::Error),

        #[error("error parsing file for {0} to utf8")]
        Utf8(String, #[source] std::string::FromUtf8Error),
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum ConfigError {
        #[error("error reading config file")]
        ReadFile(#[from] Box<ReadFile>),

        #[error("error parsing config file")]
        Parse(#[from] toml::de::Error),

        #[error("missing config file for index at {0}")]
        Missing(Box<gix::Url>),
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum AllPackagesError {
        #[error("error opening repository at {0}")]
        Open(PathBuf, #[source] Box<gix::open::Error>),

        #[error("error getting tree from repository at {0}")]
        Tree(PathBuf, #[source] Box<TreeError>),

        #[error("error decoding entry in repository at {0}")]
        Decode(PathBuf, #[source] gix::objs::decode::Error),

        #[error("error converting entry in repository at {0}")]
        Convert(PathBuf, #[source] gix::object::find::existing::Error),

        #[error("error deserializing file {0} in repository at {1}")]
        Deserialize(String, PathBuf, #[source] Box<toml::de::Error>),

        #[error("error parsing file for {0} as utf8")]
        Utf8(String, #[source] std::string::FromUtf8Error),
    }

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum DownloadError {
        #[error("error reading config file")]
        ReadFile(#[from] Box<ConfigError>),

        #[error("error downloading package")]
        Download(#[from] reqwest::Error),

        #[error("error unpacking package")]
        Unpack(#[from] std::io::Error),

        #[error("error writing index file")]
        WriteIndex(#[source] std::io::Error),

        #[error("error serializing index file")]
        SerializeIndex(#[from] toml::ser::Error),

        #[error("error deserializing index file")]
        DeserializeIndex(#[from] toml::de::Error),

        #[error("error reading index file")]
        ReadIndex(#[source] std::io::Error),
    }
}
