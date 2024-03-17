use std::path::PathBuf;

use keyring::Entry;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::cli::INDEX_DIR;

pub trait ApiTokenSource: Send + Sync {
    fn get_api_token(&self) -> anyhow::Result<Option<String>>;
    fn set_api_token(&self, api_token: &str) -> anyhow::Result<()>;
    fn delete_api_token(&self) -> anyhow::Result<()>;
    fn persists(&self) -> bool {
        true
    }
}

pub struct EnvVarApiTokenSource;

const API_TOKEN_ENV_VAR: &str = "PESDE_API_TOKEN";

impl ApiTokenSource for EnvVarApiTokenSource {
    fn get_api_token(&self) -> anyhow::Result<Option<String>> {
        match std::env::var(API_TOKEN_ENV_VAR) {
            Ok(token) => Ok(Some(token)),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // don't need to implement set_api_token or delete_api_token
    fn set_api_token(&self, _api_token: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn delete_api_token(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn persists(&self) -> bool {
        false
    }
}

static KEYRING_ENTRY: Lazy<Entry> =
    Lazy::new(|| Entry::new(env!("CARGO_BIN_NAME"), "api_token").unwrap());

pub struct KeyringApiTokenSource;

impl ApiTokenSource for KeyringApiTokenSource {
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

static AUTH_FILE_PATH: Lazy<PathBuf> = Lazy::new(|| INDEX_DIR.join("auth.yaml"));
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

pub struct ConfigFileApiTokenSource;

impl ApiTokenSource for ConfigFileApiTokenSource {
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

pub static API_TOKEN_SOURCE: Lazy<Box<dyn ApiTokenSource>> = Lazy::new(|| {
    let sources: Vec<Box<dyn ApiTokenSource>> = vec![
        Box::new(EnvVarApiTokenSource),
        Box::new(KeyringApiTokenSource),
        Box::new(ConfigFileApiTokenSource),
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
