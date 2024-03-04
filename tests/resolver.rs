use std::collections::BTreeSet;

use semver::Version;
use tempfile::tempdir;

use pesde::{
    dependencies::{
        registry::{RegistryDependencySpecifier, RegistryPackageRef},
        resolution::ResolvedPackage,
        DependencySpecifier, PackageRef,
    },
    manifest::{DependencyType, Manifest, Realm},
    package_name::PackageName,
    project::Project,
};
use prelude::*;

mod prelude;

#[test]
fn test_resolves_package() {
    let dir = tempdir().unwrap();
    let dir_path = dir.path().to_path_buf();
    let index = InMemoryIndex::new();

    let version_str = "0.1.0";
    let version: Version = version_str.parse().unwrap();
    let version_2_str = "0.1.1";
    let version_2: Version = version_2_str.parse().unwrap();

    let description = "test package";

    let pkg_name = PackageName::new("test", "test").unwrap();

    let pkg_manifest = Manifest {
        name: pkg_name.clone(),
        version: version.clone(),
        exports: Default::default(),
        path_style: Default::default(),
        private: true,
        realm: None,
        dependencies: vec![],
        peer_dependencies: vec![],
        description: Some(description.to_string()),
        license: None,
        authors: None,
    };

    let mut pkg_2_manifest = pkg_manifest.clone();
    pkg_2_manifest.version = version_2.clone();

    let index = index
        .with_scope(pkg_name.scope(), BTreeSet::from([0]))
        .with_package(pkg_name.scope(), pkg_manifest.into())
        .with_package(pkg_name.scope(), pkg_2_manifest.into());

    let specifier = DependencySpecifier::Registry(RegistryDependencySpecifier {
        name: pkg_name.clone(),
        version: format!("={version_str}").parse().unwrap(),
        realm: None,
    });
    let specifier_2 = DependencySpecifier::Registry(RegistryDependencySpecifier {
        name: pkg_name.clone(),
        version: format!(">{version_str}").parse().unwrap(),
        realm: None,
    });

    let user_manifest = Manifest {
        name: "test/user".parse().unwrap(),
        version: version.clone(),
        exports: Default::default(),
        path_style: Default::default(),
        private: true,
        realm: None,
        dependencies: vec![specifier.clone()],
        peer_dependencies: vec![specifier_2.clone()],
        description: Some(description.to_string()),
        license: None,
        authors: None,
    };

    let project = Project::new(&dir_path, &dir_path, index, user_manifest, None);

    let tree = project.manifest().dependency_tree(&project, false).unwrap();
    assert_eq!(tree.len(), 1);
    let versions = tree.get(&pkg_name).unwrap();
    assert_eq!(versions.len(), 2);
    let resolved_pkg = versions.get(&version).unwrap();
    assert_eq!(
        resolved_pkg,
        &ResolvedPackage {
            pkg_ref: PackageRef::Registry(RegistryPackageRef {
                name: pkg_name.clone(),
                version: version.clone(),
            }),
            specifier,
            dependencies: Default::default(),
            is_root: true,
            realm: Realm::Shared,
            dep_type: DependencyType::Normal,
        }
    );
    let resolved_pkg_2 = versions.get(&version_2).unwrap();
    assert_eq!(
        resolved_pkg_2,
        &ResolvedPackage {
            pkg_ref: PackageRef::Registry(RegistryPackageRef {
                name: pkg_name.clone(),
                version: version_2.clone(),
            }),
            specifier: specifier_2,
            dependencies: Default::default(),
            is_root: true,
            realm: Realm::Shared,
            dep_type: DependencyType::Normal,
        }
    );
}
