use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use crate::cli::KeyStore;

pub const SERVICE: &str = "rusthrc";
pub const ACCOUNT: &str = "openai-api-key";
pub const DEFAULT_MODEL: &str = "gpt-4o-mini";
pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
pub const ENV_KEY: &str = "OPENAI_API_KEY";

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Config {
    pub model: Option<String>,
    pub api_base: Option<String>,
    /// Only used when the key was stored with `--store file`.
    pub api_key: Option<String>,
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rusthrc")
        .join("config.toml")
}

pub fn load() -> Config {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|body| toml::from_str(&body).ok())
        .unwrap_or_default()
}

pub fn save(cfg: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let body = toml::to_string_pretty(cfg).context("failed to serialize config")?;
    fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

// ---- API key storage ----

pub struct StoredKey {
    pub value: String,
    pub source: &'static str,
}

pub fn store_api_key(key: &str, store: KeyStore) -> Result<&'static str> {
    match store {
        KeyStore::Keyring => {
            let entry =
                keyring::Entry::new(SERVICE, ACCOUNT).context("OS keyring is unavailable")?;
            entry
                .set_password(key)
                .context("failed to store the key in the OS keyring")?;
            Ok("the OS keyring")
        }
        KeyStore::File => {
            let mut cfg = load();
            cfg.api_key = Some(key.to_string());
            save(&cfg)?;
            Ok("the config file")
        }
    }
}

/// Look up the API key: `OPENAI_API_KEY` env var first, then the OS keyring,
/// then the config file. Storage failures are collected as warnings instead of
/// aborting, so a missing keyring still lets a config-file key work.
pub fn resolve_api_key() -> (Option<StoredKey>, Vec<String>) {
    let mut warnings = Vec::new();

    if let Ok(value) = std::env::var(ENV_KEY) {
        let value = value.trim().to_string();
        if !value.is_empty() {
            return (
                Some(StoredKey {
                    value,
                    source: "OPENAI_API_KEY",
                }),
                warnings,
            );
        }
    }

    match keyring::Entry::new(SERVICE, ACCOUNT) {
        Ok(entry) => match entry.get_password() {
            Ok(value) if !value.trim().is_empty() => {
                return (
                    Some(StoredKey {
                        value,
                        source: "OS keyring",
                    }),
                    warnings,
                );
            }
            Ok(_) => {}
            Err(keyring::Error::NoEntry) => {}
            Err(e) => warnings.push(format!("keyring read failed: {e}")),
        },
        Err(e) => warnings.push(format!("keyring unavailable: {e}")),
    }

    if let Some(value) = load().api_key {
        if !value.trim().is_empty() {
            return (
                Some(StoredKey {
                    value,
                    source: "config file",
                }),
                warnings,
            );
        }
    }

    (None, warnings)
}

pub fn clear_api_key() -> Result<Vec<String>> {
    let mut notes = Vec::new();
    let mut removed = false;

    match keyring::Entry::new(SERVICE, ACCOUNT) {
        Ok(entry) => match entry.delete_credential() {
            Ok(()) => {
                removed = true;
                notes.push("removed key from the OS keyring".to_string());
            }
            Err(keyring::Error::NoEntry) => {}
            Err(e) => notes.push(format!("could not clear the keyring entry: {e}")),
        },
        Err(e) => notes.push(format!("keyring unavailable: {e}")),
    }

    let mut cfg = load();
    if cfg.api_key.take().is_some() {
        save(&cfg)?;
        removed = true;
        notes.push("removed key from the config file".to_string());
    }

    if removed {
        Ok(notes)
    } else {
        Err(anyhow!("no stored API key found"))
    }
}
