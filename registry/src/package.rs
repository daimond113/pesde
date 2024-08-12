use chrono::{DateTime, Utc};
use pesde::{
    manifest::target::{Target, TargetKind},
    names::PackageName,
    source::version_id::VersionId,
};
use serde::Serialize;
use std::time::Duration;

pub const S3_SIGN_DURATION: Duration = Duration::from_secs(60 * 60);

pub fn s3_name(name: &PackageName, version_id: &VersionId) -> String {
    format!("{}+{}.tar.gz", name.escaped(), version_id.escaped())
}

#[derive(Debug, Serialize)]
pub struct TargetInfo {
    kind: TargetKind,
    lib: bool,
    bin: bool,
}

impl From<Target> for TargetInfo {
    fn from(target: Target) -> Self {
        TargetInfo {
            kind: target.kind(),
            lib: target.lib_path().is_some(),
            bin: target.bin_path().is_some(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PackageResponse {
    pub name: String,
    pub version: String,
    pub target: TargetInfo,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub published_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub license: String,
}
