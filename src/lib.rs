#![deny(missing_docs)]
//! pesde is a package manager for Roblox that is designed to be feature-rich and easy to use.
//! Currently, pesde is in a very early stage of development, but already supports the following features:
//! - Managing dependencies
//! - Re-exporting types
//! - `bin` exports (ran with Lune)
//! - Patching packages

/// Resolving, downloading and managing dependencies
pub mod dependencies;
/// Managing the pesde index
pub mod index;
/// Creating linking files ('re-export' modules)
pub mod linking_file;
/// Managing the pesde manifest
pub mod manifest;
/// Multi-threading utilities
pub mod multithread;
/// Creating, parsing, and validating package names
pub mod package_name;
/// Managing patches
pub mod patches;
/// Managing pesde projects
pub mod project;

/// The folder that contains shared packages
pub const PACKAGES_FOLDER: &str = "packages";
/// The folder that contains dev packages
pub const DEV_PACKAGES_FOLDER: &str = "dev_packages";
/// The folder that contains server packages
pub const SERVER_PACKAGES_FOLDER: &str = "server_packages";
/// The folder that contains the packages index (where every package is stored after being downloaded)
pub const INDEX_FOLDER: &str = "pesde_index";
/// The name of the manifest file
pub const MANIFEST_FILE_NAME: &str = "pesde.yaml";
/// The name of the lockfile
pub const LOCKFILE_FILE_NAME: &str = "pesde-lock.yaml";
/// The name of the patches folder
pub const PATCHES_FOLDER: &str = "patches";
/// Files to be ignored when publishing
pub const IGNORED_FOLDERS: &[&str] = &[
    PACKAGES_FOLDER,
    DEV_PACKAGES_FOLDER,
    SERVER_PACKAGES_FOLDER,
    ".git",
];

const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
