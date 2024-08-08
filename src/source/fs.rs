use std::{
    collections::BTreeMap,
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use relative_path::RelativePathBuf;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::util::hash;

/// A file system entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FSEntry {
    /// A file with the given hash
    #[serde(rename = "f")]
    File(String),
    /// A directory
    #[serde(rename = "d")]
    Directory,
}

/// A package's file system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PackageFS(pub(crate) BTreeMap<RelativePathBuf, FSEntry>);

pub(crate) fn store_in_cas<P: AsRef<Path>>(
    cas_dir: P,
    contents: &[u8],
) -> std::io::Result<(String, PathBuf)> {
    let hash = hash(contents);
    let (prefix, rest) = hash.split_at(2);

    let folder = cas_dir.as_ref().join(prefix);
    std::fs::create_dir_all(&folder)?;

    let cas_path = folder.join(rest);
    if !cas_path.exists() {
        std::fs::write(&cas_path, contents)?;
    }

    Ok((hash, cas_path))
}

pub(crate) fn store_reader_in_cas<P: AsRef<Path>>(
    cas_dir: P,
    contents: &mut dyn Read,
) -> std::io::Result<String> {
    let tmp_dir = cas_dir.as_ref().join(".tmp");
    std::fs::create_dir_all(&tmp_dir)?;
    let mut hasher = Sha256::new();
    let mut buf = [0; 8 * 1024];
    let mut file_writer = BufWriter::new(tempfile::NamedTempFile::new_in(&tmp_dir)?);

    loop {
        let bytes_read = contents.read(&mut buf)?;
        if bytes_read == 0 {
            break;
        }

        let bytes = &buf[..bytes_read];
        hasher.update(bytes);
        file_writer.write_all(bytes)?;
    }

    let hash = format!("{:x}", hasher.finalize());
    let (prefix, rest) = hash.split_at(2);

    let folder = cas_dir.as_ref().join(prefix);
    std::fs::create_dir_all(&folder)?;

    let cas_path = folder.join(rest);
    match file_writer.into_inner()?.persist_noclobber(cas_path) {
        Ok(_) => {}
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.error),
    };

    Ok(hash)
}

impl PackageFS {
    /// Write the package to the given destination
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

    /// Returns the contents of the file with the given hash
    pub fn read_file<P: AsRef<Path>, H: AsRef<str>>(
        &self,
        file_hash: H,
        cas_path: P,
    ) -> Option<String> {
        let (prefix, rest) = file_hash.as_ref().split_at(2);
        let cas_file_path = cas_path.as_ref().join(prefix).join(rest);
        std::fs::read_to_string(cas_file_path).ok()
    }
}
