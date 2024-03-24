use std::path::PathBuf;

use crate::cli::DEFAULT_INDEX_DATA;
use keyring::Entry;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

struct EnvVarApiTokenSource;

const API_TOKEN_ENV_VAR: &str = "PESDE_API_TOKEN";

impl EnvVarApiTokenSource {
    fn get_api_token(&self) -> anyhow::Result<Option<String>> {
        match std::env::var(API_TOKEN_ENV_VAR) {
            Ok(token) => Ok(Some(token)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

static AUTH_FILE_PATH: Lazy<PathBuf> =
    Lazy::new(|| DEFAULT_INDEX_DATA.0.parent().unwrap().join("auth.yaml"));
static AUTH_FILE: Lazy<AuthFile> =
    Lazy::new(
        || match std::fs::read_to_string(AUTH_FILE_PATH.to_path_buf()) {
            Ok(config) => serde_yaml::from_str(&config).unwrap(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AuthFile::default(),
            Err(e) => panic!("{:?}", e),
        },
    );

#[derive(Serialize, Deserialize, Default, Clone)]
struct AuthFile {
    #[serde(default)]
    api_token: Option<String>,
}

struct ConfigFileApiTokenSource;

impl ConfigFileApiTokenSource {
    fn get_api_token(&self) -> anyhow::Result<Option<String>> {
        Ok(AUTH_FILE.api_token.clone())
    }

    fn set_api_token(&self, api_token: &str) -> anyhow::Result<()> {
        let mut config = AUTH_FILE.clone();
        config.api_token = Some(api_token.to_string());

        serde_yaml::to_writer(
            &mut std::fs::File::create(AUTH_FILE_PATH.to_path_buf())?,
            &config,
        )?;

        Ok(())
    }

    fn delete_api_token(&self) -> anyhow::Result<()> {
        let mut config = AUTH_FILE.clone();

        config.api_token = None;

        serde_yaml::to_writer(
            &mut std::fs::File::create(AUTH_FILE_PATH.to_path_buf())?,
            &config,
        )?;

        Ok(())
    }
}

static KEYRING_ENTRY: Lazy<Entry> =
    Lazy::new(|| Entry::new(env!("CARGO_PKG_NAME"), "api_token").unwrap());

struct KeyringApiTokenSource;

impl KeyringApiTokenSource {
    fn get_api_token(&self) -> anyhow::Result<Option<String>> {
        match KEYRING_ENTRY.get_password() {
            Ok(api_token) => Ok(Some(api_token)),
            Err(err) => match err {
                keyring::Error::NoEntry | keyring::Error::PlatformFailure(_) => Ok(None),
                _ => Err(err.into()),
            },
        }
    }

    fn set_api_token(&self, api_token: &str) -> anyhow::Result<()> {
        KEYRING_ENTRY.set_password(api_token)?;

        Ok(())
    }

    fn delete_api_token(&self) -> anyhow::Result<()> {
        KEYRING_ENTRY.delete_password()?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum ApiTokenSource {
    EnvVar,
    ConfigFile,
    Keyring,
}

impl ApiTokenSource {
    pub fn get_api_token(&self) -> anyhow::Result<Option<String>> {
        match self {
            ApiTokenSource::EnvVar => EnvVarApiTokenSource.get_api_token(),
            ApiTokenSource::ConfigFile => ConfigFileApiTokenSource.get_api_token(),
            ApiTokenSource::Keyring => KeyringApiTokenSource.get_api_token(),
        }
    }

    pub fn set_api_token(&self, api_token: &str) -> anyhow::Result<()> {
        match self {
            ApiTokenSource::EnvVar => Ok(()),
            ApiTokenSource::ConfigFile => ConfigFileApiTokenSource.set_api_token(api_token),
            ApiTokenSource::Keyring => KeyringApiTokenSource.set_api_token(api_token),
        }
    }

    pub fn delete_api_token(&self) -> anyhow::Result<()> {
        match self {
            ApiTokenSource::EnvVar => Ok(()),
            ApiTokenSource::ConfigFile => ConfigFileApiTokenSource.delete_api_token(),
            ApiTokenSource::Keyring => KeyringApiTokenSource.delete_api_token(),
        }
    }

    fn persists(&self) -> bool {
        !matches!(self, ApiTokenSource::EnvVar)
    }
}

pub static API_TOKEN_SOURCE: Lazy<ApiTokenSource> = Lazy::new(|| {
    let sources: [ApiTokenSource; 3] = [
        ApiTokenSource::EnvVar,
        ApiTokenSource::ConfigFile,
        ApiTokenSource::Keyring,
    ];

    let mut valid_sources = vec![];

    for source in sources {
        match source.get_api_token() {
            Ok(Some(_)) => return source,
            Ok(None) => {
                if source.persists() {
                    valid_sources.push(source);
                }
            }
            Err(e) => {
                log::error!("error getting api token: {e}");
            }
        }
    }

    valid_sources.pop().unwrap()
});
