use crate::util::hash;
use relative_path::RelativePathBuf;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FSEntry {
    #[serde(rename = "f")]
    File(String),
    #[serde(rename = "d")]
    Directory,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PackageFS(pub(crate) BTreeMap<RelativePathBuf, FSEntry>);

pub(crate) fn store_in_cas<P: AsRef<Path>>(
    cas_dir: P,
    contents: &str,
) -> std::io::Result<(String, PathBuf)> {
    let hash = hash(contents.as_bytes());
    let (prefix, rest) = hash.split_at(2);

    let folder = cas_dir.as_ref().join(prefix);
    std::fs::create_dir_all(&folder)?;

    let cas_path = folder.join(rest);
    if !cas_path.exists() {
        std::fs::write(&cas_path, contents)?;
    }

    Ok((hash, cas_path))
}

impl PackageFS {
    pub fn write_to<P: AsRef<Path>, Q: AsRef<Path>>(
        &self,
        destination: P,
        cas_path: Q,
        link: bool,
    ) -> std::io::Result<()> {
        for (path, entry) in &self.0 {
            let path = path.to_path(destination.as_ref());

            match entry {
                FSEntry::File(hash) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }

                    let (prefix, rest) = hash.split_at(2);
                    let cas_file_path = cas_path.as_ref().join(prefix).join(rest);

                    if link {
                        std::fs::hard_link(cas_file_path, path)?;
                    } else {
                        std::fs::copy(cas_file_path, path)?;
                    }
                }
                FSEntry::Directory => {
                    std::fs::create_dir_all(path)?;
                }
            }
        }

        Ok(())
    }
}
