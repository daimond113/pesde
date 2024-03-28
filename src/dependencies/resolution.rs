use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    fmt::Display,
    path::{Path, PathBuf},
};

use log::debug;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    dependencies::{
        git::{GitDownloadError, GitPackageRef},
        registry::RegistryPackageRef,
        DependencySpecifier, PackageRef,
    },
    index::{Index, IndexFileEntry, IndexPackageError},
    manifest::{DependencyType, Manifest, OverrideKey, Realm},
    package_name::PackageName,
    project::{get_index, get_index_by_url, Project, ReadLockfileError},
    DEV_PACKAGES_FOLDER, INDEX_FOLDER, PACKAGES_FOLDER, SERVER_PACKAGES_FOLDER,
};

/// A mapping of packages to something
pub type PackageMap<T> = BTreeMap<PackageName, BTreeMap<Version, T>>;

/// The root node of the dependency graph
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, Default)]
#[serde(deny_unknown_fields)]
pub struct RootLockfileNode {
    /// Dependency overrides
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<OverrideKey, DependencySpecifier>,

    /// The specifiers of the root packages
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub specifiers: PackageMap<(DependencySpecifier, String)>,

    /// All nodes in the dependency graph
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub children: PackageMap<ResolvedPackage>,
}

impl RootLockfileNode {
    /// Returns the specifier of the root package
    pub fn root_specifier(
        &self,
        resolved_package: &ResolvedPackage,
    ) -> Option<&(DependencySpecifier, String)> {
        self.specifiers
            .get(&resolved_package.pkg_ref.name())
            .and_then(|versions| versions.get(resolved_package.pkg_ref.version()))
    }
}

/// A node in the dependency graph
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPackage {
    /// The reference to the package
    pub pkg_ref: PackageRef,
    /// The dependencies of the package
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<PackageName, (Version, String)>,
    /// The realm of the package
    pub realm: Realm,
    /// The type of the dependency
    #[serde(default, skip_serializing_if = "crate::is_default")]
    pub dep_type: DependencyType,
}

impl Display for ResolvedPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pkg_ref)
    }
}

pub(crate) fn packages_folder<'a>(realm: Realm) -> &'a str {
    match realm {
        Realm::Shared => PACKAGES_FOLDER,
        Realm::Server => SERVER_PACKAGES_FOLDER,
        Realm::Development => DEV_PACKAGES_FOLDER,
    }
}

impl ResolvedPackage {
    pub(crate) fn packages_folder(&self) -> &str {
        packages_folder(self.realm)
    }

    /// Returns the directory of the package in the project, and the parent of the directory
    pub fn directory<P: AsRef<Path>>(&self, project_path: P) -> (PathBuf, PathBuf) {
        let name = self.pkg_ref.name().escaped();
        let container_path = project_path
            .as_ref()
            .join(self.packages_folder())
            .join(INDEX_FOLDER)
            .join(&name)
            .join(self.pkg_ref.version().to_string());

        (container_path.clone(), container_path.join(&name))
    }
}

macro_rules! find_highest {
    ($iter:expr, $version:expr) => {
        $iter
            .filter(|v| $version.matches(v))
            .max_by(|a, b| a.cmp(&b))
            .cloned()
    };
}

fn find_version_from_index(
    root: &mut RootLockfileNode,
    index: &dyn Index,
    specifier: &DependencySpecifier,
    name: PackageName,
    version_req: &VersionReq,
) -> Result<IndexFileEntry, ResolveError> {
    let index_entries = index
        .package(&name)
        .map_err(|e| ResolveError::IndexPackage(e, name.to_string()))?
        .ok_or_else(|| ResolveError::PackageNotFound(name.to_string()))?;

    let resolved_versions = root.children.entry(name).or_default();

    // try to find the highest already downloaded version that satisfies the requirement, otherwise find the highest satisfying version in the index
    let Some(version) = find_highest!(resolved_versions.keys(), version_req)
        .or_else(|| find_highest!(index_entries.iter().map(|v| &v.version), version_req))
    else {
        return Err(ResolveError::NoSatisfyingVersion(Box::new(
            specifier.clone(),
        )));
    };

    Ok(index_entries
        .into_iter()
        .find(|e| e.version.eq(&version))
        .unwrap())
}

fn find_realm(a: &Realm, b: &Realm) -> Realm {
    if a == b {
        return *a;
    }

    Realm::Shared
}

/// An error that occurred while resolving dependencies
#[derive(Debug, Error)]
pub enum ResolveError {
    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred because a registry dependency conflicts with a git dependency
    #[error("registry dependency {0}@{1} conflicts with git dependency")]
    RegistryConflict(String, Version),

    /// An error that occurred because a git dependency conflicts with a registry dependency
    #[error("git dependency {0}@{1} conflicts with registry dependency")]
    GitConflict(String, Version),

    /// An error that occurred because no satisfying version was found for a dependency
    #[error("no satisfying version found for dependency {0:?}")]
    NoSatisfyingVersion(Box<DependencySpecifier>),

    /// An error that occurred while downloading a package from a git repository
    #[error("error downloading git package")]
    GitDownload(#[from] GitDownloadError),

    /// An error that occurred because a package was not found in the index
    #[error("package {0} not found in index")]
    PackageNotFound(String),

    /// An error that occurred while getting a package from the index
    #[error("failed to get package {1} from index")]
    IndexPackage(#[source] IndexPackageError, String),

    /// An error that occurred while reading the lockfile
    #[error("failed to read lockfile")]
    LockfileRead(#[from] ReadLockfileError),

    /// An error that occurred because the lockfile is out of date
    #[error("out of date lockfile")]
    OutOfDateLockfile,

    /// An error that occurred because two realms are incompatible
    #[error("incompatible realms for package {0} (package specified {1}, user specified {2})")]
    IncompatibleRealms(String, Realm, Realm),

    /// An error that occurred because a peer dependency is not installed
    #[error("peer dependency {0}@{1} is not installed")]
    PeerNotInstalled(String, Version),

    /// An error that occurred while cloning a wally index
    #[cfg(feature = "wally")]
    #[error("error cloning wally index")]
    CloneWallyIndex(#[from] crate::dependencies::wally::CloneWallyIndexError),

    /// An error that occurred while parsing a URL
    #[error("error parsing URL")]
    UrlParse(#[from] url::ParseError),
}

impl Manifest {
    fn missing_dependencies(
        &self,
        root: &mut RootLockfileNode,
        locked: bool,
        project: &Project,
    ) -> Result<BTreeMap<String, (DependencySpecifier, DependencyType)>, ResolveError> {
        Ok(if let Some(old_root) = project.lockfile()? {
            if self.overrides != old_root.overrides {
                // TODO: resolve only the changed dependencies (will this be worth it?)
                debug!("overrides have changed, resolving all dependencies");
                return Ok(self.dependencies());
            }

            debug!("lockfile found, resolving dependencies from it");
            let mut missing = BTreeMap::new();

            let current_dependencies = self.dependencies();
            let current_specifiers = current_dependencies
                .clone()
                .into_iter()
                .map(|(desired_name, (specifier, _))| (specifier, desired_name))
                .collect::<HashMap<_, _>>();

            // populate the new lockfile with all root dependencies (and their dependencies) from the old lockfile
            for (name, versions) in &old_root.children {
                for (version, resolved_package) in versions {
                    let Some((old_specifier, desired_name)) = old_root
                        .root_specifier(resolved_package)
                        .and_then(|(old_specifier, _)| {
                            current_specifiers
                                .get(old_specifier)
                                .map(|desired_name| (old_specifier, desired_name))
                        })
                    else {
                        continue;
                    };

                    root.specifiers.entry(name.clone()).or_default().insert(
                        version.clone(),
                        (old_specifier.clone(), desired_name.clone()),
                    );

                    let mut queue = VecDeque::from([(resolved_package, 0usize)]);

                    while let Some((resolved_package, depth)) = queue.pop_front() {
                        debug!(
                            "{}resolved {resolved_package} from lockfile",
                            "\t".repeat(depth)
                        );

                        root.children
                            .entry(resolved_package.pkg_ref.name())
                            .or_default()
                            .insert(
                                resolved_package.pkg_ref.version().clone(),
                                resolved_package.clone(),
                            );

                        for (dep_name, (dep_version, _)) in &resolved_package.dependencies {
                            if root
                                .children
                                .get(dep_name)
                                .and_then(|v| v.get(dep_version))
                                .is_some()
                            {
                                continue;
                            }

                            let Some(dep) = old_root
                                .children
                                .get(dep_name)
                                .and_then(|v| v.get(dep_version))
                            else {
                                return Err(ResolveError::OutOfDateLockfile);
                            };

                            queue.push_back((dep, depth + 1));
                        }
                    }
                }
            }

            let old_specifiers = old_root
                .specifiers
                .values()
                .flat_map(|v| v.values())
                .map(|(specifier, _)| specifier)
                .collect::<HashSet<_>>();

            // resolve new, or modified, dependencies from the manifest
            for (desired_name, (specifier, dep_type)) in current_dependencies {
                if old_specifiers.contains(&specifier) {
                    continue;
                }

                if locked {
                    return Err(ResolveError::OutOfDateLockfile);
                }

                missing.insert(desired_name, (specifier.clone(), dep_type));
            }

            debug!(
                "resolved {} dependencies from lockfile. new dependencies: {}",
                old_root.children.len(),
                missing.len()
            );

            missing
        } else {
            debug!("no lockfile found, resolving all dependencies");
            self.dependencies()
        })
    }

    /// Resolves the dependency graph for the project
    pub fn dependency_graph(
        &self,
        project: &mut Project,
        locked: bool,
    ) -> Result<RootLockfileNode, ResolveError> {
        debug!("resolving dependency graph for project {}", self.name);
        // try to reuse versions (according to semver specifiers) to decrease the amount of downloads and storage
        let mut root = RootLockfileNode {
            overrides: self.overrides.clone(),
            ..Default::default()
        };

        let missing_dependencies = self.missing_dependencies(&mut root, locked, project)?;

        if missing_dependencies.is_empty() {
            debug!("no dependencies left to resolve, finishing...");
            return Ok(root);
        }

        let overrides = self
            .overrides
            .iter()
            .flat_map(|(k, spec)| k.0.iter().map(|path| (path, spec.clone())))
            .collect::<HashMap<_, _>>();

        debug!("resolving {} dependencies", missing_dependencies.len());

        let mut queue = missing_dependencies
            .into_iter()
            .map(|(desired_name, (specifier, dep_type))| {
                (desired_name, specifier, dep_type, None, vec![])
            })
            .collect::<VecDeque<_>>();

        while let Some((desired_name, specifier, dep_type, dependant, mut path)) = queue.pop_front()
        {
            let depth = path.len();

            let (pkg_ref, default_realm, dependencies) = match &specifier {
                DependencySpecifier::Registry(registry_dependency) => {
                    let index = if dependant.is_none() {
                        get_index(project.indices(), Some(&registry_dependency.index))
                    } else {
                        get_index_by_url(project.indices(), &registry_dependency.index.parse()?)
                    };

                    let entry = find_version_from_index(
                        &mut root,
                        index,
                        &specifier,
                        registry_dependency.name.clone().into(),
                        &registry_dependency.version,
                    )?;

                    debug!(
                        "{}resolved registry dependency {} to {}",
                        "\t".repeat(depth),
                        registry_dependency.name,
                        entry.version
                    );

                    (
                        PackageRef::Registry(RegistryPackageRef {
                            name: registry_dependency.name.clone(),
                            version: entry.version,
                            index_url: index.url().clone(),
                        }),
                        entry.realm,
                        entry.dependencies,
                    )
                }
                DependencySpecifier::Git(git_dependency) => {
                    let (manifest, url, rev) =
                        git_dependency.resolve(project.cache_dir(), project.indices())?;

                    debug!(
                        "{}resolved git dependency {} to {url}#{rev}",
                        "\t".repeat(depth),
                        git_dependency.repo
                    );

                    (
                        PackageRef::Git(GitPackageRef {
                            name: manifest.name.clone(),
                            version: manifest.version.clone(),
                            repo_url: url,
                            rev,
                        }),
                        manifest.realm,
                        manifest.dependencies(),
                    )
                }
                #[cfg(feature = "wally")]
                DependencySpecifier::Wally(wally_dependency) => {
                    let cache_dir = project.cache_dir().to_path_buf();
                    let index = crate::dependencies::wally::clone_wally_index(
                        &cache_dir,
                        project.indices_mut(),
                        &wally_dependency.index_url,
                    )?;

                    let entry = find_version_from_index(
                        &mut root,
                        &index,
                        &specifier,
                        wally_dependency.name.clone().into(),
                        &wally_dependency.version,
                    )?;

                    debug!(
                        "{}resolved wally dependency {} to {}",
                        "\t".repeat(depth),
                        wally_dependency.name,
                        entry.version
                    );

                    (
                        PackageRef::Wally(crate::dependencies::wally::WallyPackageRef {
                            name: wally_dependency.name.clone(),
                            version: entry.version,
                            index_url: index.url().clone(),
                        }),
                        entry.realm,
                        entry.dependencies,
                    )
                }
            };

            // if the dependency is a root dependency, it can be thought of as a normal dependency
            let dep_type = if dependant.is_some() {
                dep_type
            } else {
                DependencyType::Normal
            };

            let specifier_realm = specifier.realm().copied();

            if let Some((dependant_name, dependant_version)) = dependant {
                root.children
                    .get_mut(&dependant_name)
                    .and_then(|v| v.get_mut(&dependant_version))
                    .unwrap()
                    .dependencies
                    .insert(
                        pkg_ref.name(),
                        (pkg_ref.version().clone(), desired_name.clone()),
                    );
            } else {
                root.specifiers
                    .entry(pkg_ref.name())
                    .or_default()
                    .insert(pkg_ref.version().clone(), (specifier, desired_name.clone()));
            }

            let resolved_versions = root.children.entry(pkg_ref.name()).or_default();

            if let Some(previously_resolved) = resolved_versions.get_mut(pkg_ref.version()) {
                match (&pkg_ref, &previously_resolved.pkg_ref) {
                    (PackageRef::Registry(r), PackageRef::Git(_g)) => {
                        return Err(ResolveError::RegistryConflict(
                            r.name.to_string(),
                            r.version.clone(),
                        ));
                    }
                    (PackageRef::Git(g), PackageRef::Registry(_r)) => {
                        return Err(ResolveError::GitConflict(
                            g.name.to_string(),
                            g.version.clone(),
                        ));
                    }
                    _ => (),
                }

                if previously_resolved.dep_type == DependencyType::Peer
                    && dep_type == DependencyType::Normal
                {
                    previously_resolved.dep_type = dep_type;
                }

                // need not resolve the package again
                continue;
            }

            if specifier_realm.is_some_and(|realm| realm == Realm::Shared)
                && default_realm.is_some_and(|realm| realm == Realm::Server)
            {
                return Err(ResolveError::IncompatibleRealms(
                    pkg_ref.name().to_string(),
                    default_realm.unwrap(),
                    specifier_realm.unwrap(),
                ));
            }

            resolved_versions.insert(
                pkg_ref.version().clone(),
                ResolvedPackage {
                    pkg_ref: pkg_ref.clone(),
                    dependencies: Default::default(),
                    realm: specifier_realm
                        .unwrap_or_default()
                        .or(default_realm.unwrap_or_default()),
                    dep_type,
                },
            );

            path.push(desired_name);

            for (desired_name, (specifier, ty)) in dependencies {
                let overridden = overrides.iter().find_map(|(k_path, spec)| {
                    (path == k_path[..k_path.len() - 1] && k_path.last() == Some(&desired_name))
                        .then_some(spec)
                });

                queue.push_back((
                    desired_name,
                    overridden.cloned().unwrap_or(specifier),
                    ty,
                    Some((pkg_ref.name(), pkg_ref.version().clone())),
                    path.clone(),
                ));
            }
        }

        debug!("resolving realms and peer dependencies...");

        for (name, versions) in root.children.clone() {
            for (version, resolved_package) in versions {
                if resolved_package.dep_type == DependencyType::Peer {
                    return Err(ResolveError::PeerNotInstalled(
                        resolved_package.pkg_ref.name().to_string(),
                        resolved_package.pkg_ref.version().clone(),
                    ));
                }

                let mut realm = resolved_package.realm;

                for (dep_name, (dep_version, _)) in &resolved_package.dependencies {
                    let dep = root.children.get(dep_name).and_then(|v| v.get(dep_version));

                    if let Some(dep) = dep {
                        realm = find_realm(&realm, &dep.realm);
                    }
                }

                root.children
                    .get_mut(&name)
                    .and_then(|v| v.get_mut(&version))
                    .unwrap()
                    .realm = realm;
            }
        }

        debug!("finished resolving dependency graph");

        Ok(root)
    }
}
