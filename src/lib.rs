// #![deny(missing_docs)] - TODO: bring this back before publishing 0.5

#[cfg(not(any(feature = "roblox", feature = "lune", feature = "luau")))]
compile_error!("at least one of the features `roblox`, `lune`, or `luau` must be enabled");

use once_cell::sync::Lazy;
use std::path::{Path, PathBuf};

pub mod lockfile;
pub mod manifest;
pub mod names;
pub mod source;

pub const MANIFEST_FILE_NAME: &str = "pesde.yaml";
pub const LOCKFILE_FILE_NAME: &str = "pesde.lock";

pub(crate) static REQWEST_CLIENT: Lazy<reqwest::blocking::Client> = Lazy::new(|| {
    reqwest::blocking::Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .expect("failed to create reqwest client")
});

#[derive(Debug, Clone)]
pub struct GitAccount {
    username: String,
    password: secrecy::SecretString,
}

impl GitAccount {
    pub fn new<S: Into<secrecy::SecretString>>(username: String, password: S) -> Self {
        GitAccount {
            username,
            password: password.into(),
        }
    }

    pub fn as_account(&self) -> gix::sec::identity::Account {
        use secrecy::ExposeSecret;

        gix::sec::identity::Account {
            username: self.username.clone(),
            password: self.password.expose_secret().to_string(),
        }
    }
}

impl From<gix::sec::identity::Account> for GitAccount {
    fn from(account: gix::sec::identity::Account) -> Self {
        GitAccount {
            username: account.username,
            password: account.password.into(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct AuthConfig {
    pesde_token: Option<secrecy::SecretString>,
    git_credentials: Option<GitAccount>,
}

impl AuthConfig {
    pub fn new() -> Self {
        AuthConfig::default()
    }

    pub fn with_pesde_token<S: Into<secrecy::SecretString>>(mut self, token: Option<S>) -> Self {
        self.pesde_token = token.map(Into::into);
        self
    }

    pub fn with_git_credentials(mut self, git_credentials: Option<GitAccount>) -> Self {
        self.git_credentials = git_credentials;
        self
    }
}

pub(crate) fn authenticate_conn(
    conn: &mut gix::remote::Connection<
        '_,
        '_,
        Box<dyn gix::protocol::transport::client::Transport + Send>,
    >,
    auth_config: AuthConfig,
) {
    if let Some(iden) = auth_config.git_credentials {
        conn.set_credentials(move |action| match action {
            gix::credentials::helper::Action::Get(ctx) => {
                Ok(Some(gix::credentials::protocol::Outcome {
                    identity: iden.as_account(),
                    next: gix::credentials::helper::NextAction::from(ctx),
                }))
            }
            gix::credentials::helper::Action::Store(_) => Ok(None),
            gix::credentials::helper::Action::Erase(_) => Ok(None),
        });
    }
}

#[derive(Debug)]
pub struct Project {
    path: PathBuf,
    data_dir: PathBuf,
    auth_config: AuthConfig,
}

impl Project {
    pub fn new<P: AsRef<Path>, Q: AsRef<Path>>(
        path: P,
        data_dir: Q,
        auth_config: AuthConfig,
    ) -> Self {
        Project {
            path: path.as_ref().to_path_buf(),
            data_dir: data_dir.as_ref().to_path_buf(),
            auth_config,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn read_manifest(&self) -> Result<Vec<u8>, errors::ManifestReadError> {
        let bytes = std::fs::read(self.path.join(MANIFEST_FILE_NAME))?;
        Ok(bytes)
    }

    pub fn deser_manifest(&self) -> Result<manifest::Manifest, errors::ManifestReadError> {
        let bytes = std::fs::read(self.path.join(MANIFEST_FILE_NAME))?;
        Ok(serde_yaml::from_slice(&bytes)?)
    }

    pub fn write_manifest<S: AsRef<[u8]>>(&self, manifest: S) -> Result<(), std::io::Error> {
        std::fs::write(self.path.join(MANIFEST_FILE_NAME), manifest.as_ref())
    }
}

pub mod errors {
    use thiserror::Error;

    #[derive(Debug, Error)]
    #[non_exhaustive]
    pub enum ManifestReadError {
        #[error("io error reading manifest file")]
        Io(#[from] std::io::Error),

        #[error("error deserializing manifest file")]
        Serde(#[from] serde_yaml::Error),
    }
}
