//! Provider API keys stored beside `config.toml`.
//!
//! A key a person pastes in the connect screen goes here, never into
//! `config.toml`: that file is meant to be readable and shareable, and a key is
//! neither. The file holds one `[keys]` table, keyed by provider name. It is
//! written with `0600` where the platform has Unix permissions, and a load
//! failure is treated as "no stored keys" so a damaged file cannot stop the
//! agent from starting.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The credentials file that belongs beside `config_path`.
#[must_use]
pub fn path_for(config_path: &Path) -> PathBuf {
    config_path.with_file_name("credentials.toml")
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct RawCredentials {
    #[serde(default)]
    keys: BTreeMap<String, String>,
}

/// Read every stored key.
///
/// A missing, unreadable or malformed file yields no keys: credentials are an
/// enhancement, and refusing to start over one would be worse than asking for
/// the key again.
#[must_use]
pub fn load(path: &Path) -> BTreeMap<String, String> {
    let Ok(text) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    toml::from_str::<RawCredentials>(&text)
        .map(|raw| raw.keys)
        .unwrap_or_default()
}

/// Store one key, replacing any earlier value for the same provider.
///
/// # Errors
///
/// Returns the write error when the file or its directory cannot be written.
pub fn set(path: &Path, provider: &str, key: &str) -> io::Result<()> {
    let mut keys = load(path);
    if key.trim().is_empty() {
        keys.remove(provider);
    } else {
        keys.insert(provider.to_owned(), key.trim().to_owned());
    }
    save(path, &keys)
}

/// Write the whole key set, creating the parent directory and restricting the
/// file to its owner.
///
/// # Errors
///
/// Returns the write error when the file or its directory cannot be written.
pub fn save(path: &Path, keys: &BTreeMap<String, String>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let raw = RawCredentials { keys: keys.clone() };
    let text = toml::to_string(&raw).map_err(io::Error::other)?;
    write_private(path, text.as_bytes())
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_round_trips_and_an_empty_one_removes_it() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = path_for(&directory.path().join("config.toml"));

        set(&path, "groq", "  gsk_test  ").expect("set");
        assert_eq!(
            load(&path).get("groq").map(String::as_str),
            Some("gsk_test")
        );

        set(&path, "groq", "   ").expect("clear");
        assert!(load(&path).is_empty());
    }

    #[test]
    fn a_malformed_file_reads_as_empty_rather_than_failing() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("credentials.toml");
        fs::write(&path, "keys = [").expect("write");
        assert!(load(&path).is_empty());
    }

    #[test]
    fn repeated_sets_keep_the_other_providers() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = path_for(&directory.path().join("config.toml"));
        set(&path, "groq", "one").expect("set");
        set(&path, "openai", "two").expect("set");
        let keys = load(&path);
        assert_eq!(keys.get("groq").map(String::as_str), Some("one"));
        assert_eq!(keys.get("openai").map(String::as_str), Some("two"));
    }
}
