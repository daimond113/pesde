use std::num::NonZero;

use async_trait::async_trait;
use pesde::names::PackageName;
use pesde::signature::Signature;
use pesde::source::pesde::registry::*;

use crate::db::MmrWriteStore;

#[derive(Debug, thiserror::Error)]
pub enum PackageWriteError {
	#[error("the package version has already been published")]
	VersionAlreadyExists,

	#[error("the package version does not exist")]
	UnknownPackageVersion,

	#[error("the package version is already yanked")]
	AlreadyYanked,

	#[error("the package version is not yanked")]
	NotYanked,

	#[error("the package is already deprecated")]
	AlreadyDeprecated,

	#[error("the package is not deprecated")]
	NotDeprecated,

	#[error(transparent)]
	Internal(#[from] anyhow::Error),
}

#[async_trait]
pub trait Repository {}

#[async_trait]
pub trait WriteRepository {}
