use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt::Display,
    path::{Path, PathBuf},
};

use log::debug;
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    dependencies::{
        git::{GitDownloadError, GitPackageRef},
        registry::{RegistryDependencySpecifier, RegistryPackageRef},
        DependencySpecifier, PackageRef,
    },
    index::{Index, IndexPackageError},
    manifest::{DependencyType, Manifest, Realm},
    package_name::PackageName,
    project::{Project, ReadLockfileError},
    DEV_PACKAGES_FOLDER, INDEX_FOLDER, PACKAGES_FOLDER, SERVER_PACKAGES_FOLDER,
};

/// A node in the dependency tree
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPackage {
    /// The reference to the package
    pub pkg_ref: PackageRef,
    /// The specifier that resolved to this package
    pub specifier: DependencySpecifier,
    /// The dependencies of the package
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub dependencies: BTreeSet<(PackageName, Version)>,
    /// Whether the package is a root package (top-level dependency)
    pub is_root: bool,
    /// The realm of the package
    pub realm: Realm,
    /// The type of the dependency
    pub dep_type: DependencyType,
}

impl Display for ResolvedPackage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.pkg_ref)
    }
}

pub(crate) fn packages_folder(realm: &Realm) -> &str {
    match realm {
        Realm::Shared => PACKAGES_FOLDER,
        Realm::Server => SERVER_PACKAGES_FOLDER,
        Realm::Development => DEV_PACKAGES_FOLDER,
    }
}

impl ResolvedPackage {
    pub(crate) fn packages_folder(&self) -> &str {
        packages_folder(&self.realm)
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

/// A flat resolved map, a map of package names to versions to resolved packages
pub type ResolvedVersionsMap = BTreeMap<PackageName, BTreeMap<Version, ResolvedPackage>>;

macro_rules! find_highest {
    ($iter:expr, $dep:expr) => {
        $iter
            .filter(|v| $dep.version.matches(v))
            .max_by(|a, b| a.cmp(&b))
            .cloned()
    };
}

fn find_realm(a: &Realm, b: &Realm) -> Realm {
    if a == b {
        return *a;
    }

    Realm::Shared
}

fn add_to_map(
    map: &mut ResolvedVersionsMap,
    name: &PackageName,
    version: &Version,
    resolved_package: &ResolvedPackage,
    lockfile: &ResolvedVersionsMap,
    depth: usize,
) -> Result<(), ResolveError> {
    debug!(
        "{}resolved {resolved_package} from lockfile",
        "\t".repeat(depth)
    );

    map.entry(name.clone())
        .or_default()
        .insert(version.clone(), resolved_package.clone());

    for (dep_name, dep_version) in &resolved_package.dependencies {
        if map.get(dep_name).and_then(|v| v.get(dep_version)).is_none() {
            let dep = lockfile.get(dep_name).and_then(|v| v.get(dep_version));

            match dep {
                Some(dep) => add_to_map(map, dep_name, dep_version, dep, lockfile, depth + 1)?,
                // the lockfile is malformed
                None => return Err(ResolveError::OutOfDateLockfile),
            }
        }
    }

    Ok(())
}

/// An error that occurred while resolving dependencies
#[derive(Debug, Error)]
pub enum ResolveError {
    /// An error that occurred while interacting with the file system
    #[error("error interacting with the file system")]
    Io(#[from] std::io::Error),

    /// An error that occurred because a registry dependency conflicts with a git dependency
    #[error("registry dependency {0}@{1} conflicts with git dependency")]
    RegistryConflict(PackageName, Version),

    /// An error that occurred because a git dependency conflicts with a registry dependency
    #[error("git dependency {0}@{1} conflicts with registry dependency")]
    GitConflict(PackageName, Version),

    /// An error that occurred because no satisfying version was found for a dependency
    #[error("no satisfying version found for dependency {0:?}")]
    NoSatisfyingVersion(RegistryDependencySpecifier),

    /// An error that occurred while downloading a package from a git repository
    #[error("error downloading git package")]
    GitDownload(#[from] GitDownloadError),

    /// An error that occurred because a package was not found in the index
    #[error("package {0} not found in index")]
    PackageNotFound(PackageName),

    /// An error that occurred while getting a package from the index
    #[error("failed to get package {1} from index")]
    IndexPackage(#[source] IndexPackageError, PackageName),

    /// An error that occurred while reading the lockfile
    #[error("failed to read lockfile")]
    LockfileRead(#[from] ReadLockfileError),

    /// An error that occurred because the lockfile is out of date
    #[error("out of date lockfile")]
    OutOfDateLockfile,

    /// An error that occurred because two realms are incompatible
    #[error("incompatible realms for package {0} (package specified {1}, user specified {2})")]
    IncompatibleRealms(PackageName, Realm, Realm),

    /// An error that occurred because a peer dependency is not installed
    #[error("peer dependency {0}@{1} is not installed")]
    PeerNotInstalled(PackageName, Version),
}

impl Manifest {
    /// Resolves the dependency tree for the project
    pub fn dependency_tree<I: Index>(
        &self,
        project: &Project<I>,
        locked: bool,
    ) -> Result<ResolvedVersionsMap, ResolveError> {
        debug!("resolving dependency tree for project {}", self.name);
        // try to reuse versions (according to semver specifiers) to decrease the amount of downloads and storage
        let mut resolved_versions_map: ResolvedVersionsMap = BTreeMap::new();

        let tree = if let Some(lockfile) = project.lockfile()? {
            debug!("lockfile found, resolving dependencies from it");
            let mut missing = Vec::new();

            // resolve all root dependencies (and their dependencies) from the lockfile
            for (name, versions) in &lockfile {
                for (version, resolved_package) in versions {
                    if !resolved_package.is_root
                        || !self
                            .dependencies()
                            .into_iter()
                            .any(|(spec, _)| spec == resolved_package.specifier)
                    {
                        continue;
                    }

                    add_to_map(
                        &mut resolved_versions_map,
                        name,
                        version,
                        resolved_package,
                        &lockfile,
                        1,
                    )?;
                }
            }

            // resolve new, or modified, dependencies from the lockfile
            'outer: for (dep, dep_type) in self.dependencies() {
                for versions in resolved_versions_map.values() {
                    for resolved_package in versions.values() {
                        if resolved_package.specifier == dep && resolved_package.is_root {
                            continue 'outer;
                        }
                    }
                }

                if locked {
                    return Err(ResolveError::OutOfDateLockfile);
                }

                missing.push((dep.clone(), dep_type));
            }

            debug!(
                "resolved {} dependencies from lockfile. new dependencies: {}",
                resolved_versions_map.len(),
                missing.len()
            );

            missing
        } else {
            debug!("no lockfile found, resolving all dependencies");
            self.dependencies()
        };

        if tree.is_empty() {
            debug!("no dependencies left to resolve, finishing...");
            return Ok(resolved_versions_map);
        }

        debug!("resolving {} dependencies from index", tree.len());

        let mut queue = VecDeque::from_iter(self.dependencies().into_iter().map(|d| (d, None)));

        while let Some(((specifier, dep_type), dependant)) = queue.pop_front() {
            let (pkg_ref, default_realm, dependencies) = match &specifier {
                DependencySpecifier::Registry(registry_dependency) => {
                    let index_entries = project
                        .index()
                        .package(&registry_dependency.name)
                        .map_err(|e| {
                            ResolveError::IndexPackage(e, registry_dependency.name.clone())
                        })?
                        .ok_or_else(|| {
                            ResolveError::PackageNotFound(registry_dependency.name.clone())
                        })?;

                    let resolved_versions = resolved_versions_map
                        .entry(registry_dependency.name.clone())
                        .or_default();

                    // try to find the highest already downloaded version that satisfies the requirement, otherwise find the highest satisfying version in the index
                    let Some(version) =
                        find_highest!(resolved_versions.keys(), registry_dependency).or_else(
                            || {
                                find_highest!(
                                    index_entries.iter().map(|v| &v.version),
                                    registry_dependency
                                )
                            },
                        )
                    else {
                        return Err(ResolveError::NoSatisfyingVersion(
                            registry_dependency.clone(),
                        ));
                    };

                    let entry = index_entries
                        .into_iter()
                        .find(|e| e.version.eq(&version))
                        .unwrap();

                    debug!(
                        "resolved registry dependency {} to {}",
                        registry_dependency.name, version
                    );

                    (
                        PackageRef::Registry(RegistryPackageRef {
                            name: registry_dependency.name.clone(),
                            version: version.clone(),
                        }),
                        entry.realm,
                        entry.dependencies,
                    )
                }
                DependencySpecifier::Git(git_dependency) => {
                    let (manifest, url, rev) = git_dependency.resolve(project)?;

                    debug!(
                        "resolved git dependency {} to {url}#{rev}",
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
            };

            let is_root = dependant.is_none();
            // if the dependency is a root dependency, it can be thought of as a normal dependency
            let dep_type = if is_root {
                DependencyType::Normal
            } else {
                dep_type
            };

            if let Some((dependant_name, dependant_version)) = dependant {
                resolved_versions_map
                    .get_mut(&dependant_name)
                    .and_then(|v| v.get_mut(&dependant_version))
                    .unwrap()
                    .dependencies
                    .insert((pkg_ref.name().clone(), pkg_ref.version().clone()));
            }

            let resolved_versions = resolved_versions_map
                .entry(pkg_ref.name().clone())
                .or_default();

            if let Some(previously_resolved) = resolved_versions.get_mut(pkg_ref.version()) {
                match (&pkg_ref, &previously_resolved.pkg_ref) {
                    (PackageRef::Registry(r), PackageRef::Git(_g)) => {
                        return Err(ResolveError::RegistryConflict(
                            r.name.clone(),
                            r.version.clone(),
                        ));
                    }
                    (PackageRef::Git(g), PackageRef::Registry(_r)) => {
                        return Err(ResolveError::GitConflict(g.name.clone(), g.version.clone()));
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

            if specifier
                .realm()
                .is_some_and(|realm| realm == &Realm::Shared)
                && default_realm.is_some_and(|realm| realm == Realm::Server)
            {
                return Err(ResolveError::IncompatibleRealms(
                    pkg_ref.name().clone(),
                    default_realm.unwrap(),
                    *specifier.realm().unwrap(),
                ));
            }

            resolved_versions.insert(
                pkg_ref.version().clone(),
                ResolvedPackage {
                    pkg_ref: pkg_ref.clone(),
                    specifier: specifier.clone(),
                    dependencies: BTreeSet::new(),
                    is_root,
                    realm: *specifier
                        .realm()
                        .copied()
                        .unwrap_or_default()
                        .or(&default_realm.unwrap_or_default()),
                    dep_type,
                },
            );

            for dependency in dependencies {
                queue.push_back((
                    dependency,
                    Some((pkg_ref.name().clone(), pkg_ref.version().clone())),
                ));
            }
        }

        debug!("resolving realms and peer dependencies...");

        for (name, versions) in resolved_versions_map.clone() {
            for (version, resolved_package) in versions {
                if resolved_package.dep_type == DependencyType::Peer {
                    return Err(ResolveError::PeerNotInstalled(
                        resolved_package.pkg_ref.name().clone(),
                        resolved_package.pkg_ref.version().clone(),
                    ));
                }

                let mut realm = resolved_package.realm;

                for (dep_name, dep_version) in &resolved_package.dependencies {
                    let dep = resolved_versions_map
                        .get(dep_name)
                        .and_then(|v| v.get(dep_version));

                    if let Some(dep) = dep {
                        realm = find_realm(&realm, &dep.realm);
                    }
                }

                resolved_versions_map
                    .get_mut(&name)
                    .and_then(|v| v.get_mut(&version))
                    .unwrap()
                    .realm = realm;
            }
        }

        debug!("finished resolving dependency tree");

        Ok(resolved_versions_map)
    }
}
