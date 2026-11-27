//! Persistent workspace trust for project-provided content.
//!
//! A workspace is trusted when the person said so once; the decision is stored
//! beside `config.toml` in `trusted_workspaces.toml`, keyed by the canonical
//! workspace path. Project skills, project hooks and project instruction files
//! are admitted only when [`TrustStore::is_trusted`] says the workspace is
//! trusted. User-level and built-in content is never gated.
//!
//! Everything here fails closed. A missing store, an unreadable store, a store
//! written by a version this build does not understand, and an entry that does
//! not parse all read as "no workspace is trusted". Nothing in this module
//! panics on a malformed file.
//!
//! A key is the canonical path, so `/a/b`, `/a/b/` and a symlink to `/a/b`
//! resolve to the same entry. Matching is exact: `/a/bc` is not trusted by an
//! entry for `/a/b`, and a child directory does not inherit its parent's trust.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config_home;

/// The file that holds trust decisions, beside `config.toml`.
pub const STORE_FILE: &str = "trusted_workspaces.toml";

/// The schema version this build writes and reads. A store with another
/// version is ignored rather than guessed at.
const STORE_VERSION: u32 = 1;

/// The trust posture a caller applies to project content.
///
/// [`ProjectTrust::Untrusted`] is the default, so a value that was never
/// decided suppresses project content instead of admitting it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProjectTrust {
    /// The workspace root was trusted, or the caller already made that decision.
    Trusted,
    /// Project content must not be admitted.
    #[default]
    Untrusted,
}

impl ProjectTrust {
    /// Whether project content may be admitted.
    #[must_use]
    pub fn is_trusted(self) -> bool {
        matches!(self, Self::Trusted)
    }
}

/// One persisted trust record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrustEntry {
    /// Unix seconds when the workspace was trusted. Missing in a hand-written
    /// file and never used to decide trust.
    pub trusted_at: Option<u64>,
}

/// The persisted trust decisions for this profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustStore {
    path: PathBuf,
    projects: BTreeMap<String, TrustEntry>,
}

/// The on-disk shape. Read and write share it so the two cannot drift.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct RawStore {
    version: u32,
    projects: BTreeMap<String, TrustEntry>,
}

impl TrustStore {
    /// The default store path: `trusted_workspaces.toml` in [`config_home`].
    #[must_use]
    pub fn default_path() -> PathBuf {
        config_home().join(STORE_FILE)
    }

    /// Load the default store, tolerating every failure as "nothing trusted".
    #[must_use]
    pub fn load() -> Self {
        Self::load_from(&Self::default_path())
    }

    /// Load `path`, tolerating every failure as "nothing trusted".
    ///
    /// A missing file, an unreadable file, invalid TOML, or a store written by
    /// another schema version yields an empty store. A malformed entry drops
    /// every entry rather than guessing which ones are sound.
    #[must_use]
    pub fn load_from(path: &Path) -> Self {
        let mut store = Self::at(path);
        store.projects = read_projects(path).unwrap_or_default();
        store
    }

    /// An empty store bound to `path`, without reading it.
    ///
    /// A test or a host that keeps its own store uses this; `trust` and
    /// `untrust` still write to `path`.
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            projects: BTreeMap::new(),
        }
    }

    /// The file this store reads and writes.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the canonical form of `workspace` has a persisted grant.
    ///
    /// Matching is exact. A workspace that never existed is not trusted, and a
    /// path that merely shares a prefix with a trusted one is not trusted.
    #[must_use]
    pub fn is_trusted(&self, workspace: &Path) -> bool {
        self.projects.contains_key(&workspace_key(workspace))
    }

    /// This store's decision for `workspace`.
    #[must_use]
    pub fn decision(&self, workspace: &Path) -> ProjectTrust {
        if self.is_trusted(workspace) {
            ProjectTrust::Trusted
        } else {
            ProjectTrust::Untrusted
        }
    }

    /// Record a grant for the canonical form of `workspace`.
    ///
    /// Entries written by another process since this store was loaded are
    /// kept: the file is re-read and merged before the new entry is written.
    ///
    /// # Errors
    ///
    /// Returns [`TrustError::Write`] when the store cannot be written, and
    /// [`TrustError::Encode`] when it cannot be serialised.
    pub fn trust(&mut self, workspace: &Path) -> Result<(), TrustError> {
        let mut projects = self.reload();
        projects.insert(
            workspace_key(workspace),
            TrustEntry {
                trusted_at: now_seconds(),
            },
        );
        self.projects = projects;
        self.save()
    }

    /// Remove any grant for the canonical form of `workspace`.
    ///
    /// Returns whether an entry was removed. Removing a workspace that was not
    /// trusted writes nothing, so a second [`TrustStore::untrust`] is a no-op.
    ///
    /// # Errors
    ///
    /// Returns [`TrustError::Write`] when the store cannot be written, and
    /// [`TrustError::Encode`] when it cannot be serialised.
    pub fn untrust(&mut self, workspace: &Path) -> Result<bool, TrustError> {
        let mut projects = self.reload();
        if projects.remove(&workspace_key(workspace)).is_none() {
            self.projects = projects;
            return Ok(false);
        }
        self.projects = projects;
        self.save()?;
        Ok(true)
    }

    /// Every trusted workspace, in path order, for a caller that displays them.
    #[must_use]
    pub fn list_trusted(&self) -> Vec<PathBuf> {
        self.projects.keys().map(PathBuf::from).collect()
    }

    /// Whether the store holds no grant.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.projects.is_empty()
    }

    /// Read the entries currently on disk, falling back to the in-memory copy
    /// only when the file is missing or unusable.
    fn reload(&self) -> BTreeMap<String, TrustEntry> {
        read_projects(&self.path).unwrap_or_else(|| self.projects.clone())
    }

    fn save(&self) -> Result<(), TrustError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| TrustError::Write {
                path: self.path.clone(),
                source,
            })?;
        }
        let text = toml::to_string(&RawStore {
            version: STORE_VERSION,
            projects: self.projects.clone(),
        })?;
        let temporary = temporary_path(&self.path);
        let result = write_private(&temporary, text.as_bytes())
            .and_then(|()| fs::rename(&temporary, &self.path));
        match result {
            Ok(()) => Ok(()),
            Err(source) => {
                // Best effort: leave no half-written temporary behind.
                let _ = fs::remove_file(&temporary);
                Err(TrustError::Write {
                    path: self.path.clone(),
                    source,
                })
            }
        }
    }
}

/// Why a trust decision could not be persisted.
#[derive(Debug, thiserror::Error)]
pub enum TrustError {
    /// The store file could not be written.
    #[error("cannot write trust store {path}: {source}")]
    Write {
        /// The store path.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// The in-memory store could not be encoded as TOML.
    #[error("cannot encode trust store: {0}")]
    Encode(#[from] toml::ser::Error),
}

/// Read and validate the store at `path`.
///
/// `None` means missing, unreadable, invalid TOML, or another schema version;
/// every caller treats those as "nothing trusted".
fn read_projects(path: &Path) -> Option<BTreeMap<String, TrustEntry>> {
    let text = fs::read_to_string(path).ok()?;
    let raw: RawStore = toml::from_str(&text).ok()?;
    (raw.version == STORE_VERSION).then_some(raw.projects)
}

/// The canonical key for `workspace`.
///
/// An existing path is resolved through its symlinks. A path that does not
/// exist yet is made absolute and cleaned lexically, so a later `trust` call
/// with the same spelling agrees.
fn workspace_key(workspace: &Path) -> String {
    let resolved = match workspace.canonicalize() {
        Ok(path) => path,
        Err(_) => clean_absolute(workspace),
    };
    resolved.to_string_lossy().into_owned()
}

/// Make `path` absolute without touching the filesystem, then remove `.` and
/// resolve `..` lexically.
fn clean_absolute(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut cleaned = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !cleaned.pop() && !cleaned.has_root() {
                    cleaned.push("..");
                }
            }
            other => cleaned.push(other.as_os_str()),
        }
    }
    cleaned
}

fn now_seconds() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_secs())
}

fn temporary_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(STORE_FILE);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    path.with_file_name(format!(".{name}.{}.{nanos}.tmp", std::process::id()))
}

fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_path(directory: &Path) -> PathBuf {
        directory.join(STORE_FILE)
    }

    fn workspace(directory: &Path, name: &str) -> PathBuf {
        let path = directory.join(name);
        fs::create_dir_all(&path).expect("workspace");
        path
    }

    #[test]
    fn trust_persists_and_loads_from_disk() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = store_path(directory.path());
        let workspace = workspace(directory.path(), "repo");

        let mut store = TrustStore::at(&path);
        assert!(!store.is_trusted(&workspace));
        store.trust(&workspace).expect("trust");
        assert!(store.is_trusted(&workspace));

        let reloaded = TrustStore::load_from(&path);
        assert!(reloaded.is_trusted(&workspace));
        assert_eq!(
            reloaded.list_trusted(),
            vec![workspace.canonicalize().expect("canonical")]
        );
        assert_eq!(reloaded.decision(&workspace), ProjectTrust::Trusted);
        assert_eq!(reloaded.decision(directory.path()), ProjectTrust::Untrusted);
    }

    #[test]
    fn untrust_removes_the_entry_and_is_idempotent() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = store_path(directory.path());
        let workspace = workspace(directory.path(), "repo");

        let mut store = TrustStore::at(&path);
        store.trust(&workspace).expect("trust");
        assert!(store.untrust(&workspace).expect("untrust"));
        assert!(!store.is_trusted(&workspace));
        assert!(!store.untrust(&workspace).expect("untrust again"));

        let reloaded = TrustStore::load_from(&path);
        assert!(reloaded.is_empty());
        assert!(!reloaded.is_trusted(&workspace));
    }

    #[cfg(unix)]
    #[test]
    fn trailing_slash_and_symlinks_resolve_to_one_key() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("tempdir");
        let store_path = store_path(directory.path());
        let workspace = workspace(directory.path(), "repo");
        let link = directory.path().join("link");
        symlink(&workspace, &link).expect("symlink");

        let mut store = TrustStore::at(&store_path);
        store.trust(&workspace).expect("trust");

        assert!(store.is_trusted(&link), "a symlink resolves to the target");
        assert!(store.is_trusted(&workspace));
        // The same path written with a trailing separator names one key.
        let trailing = PathBuf::from(format!("{}/", workspace.display()));
        assert!(store.is_trusted(&trailing));
        assert_eq!(store.list_trusted().len(), 1);
    }

    #[test]
    fn a_prefix_is_not_a_grant() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = store_path(directory.path());
        let trusted = workspace(directory.path(), "a/b");
        let sibling = workspace(directory.path(), "a/bc");
        let child = workspace(directory.path(), "a/b/nested");

        let mut store = TrustStore::at(&path);
        store.trust(&trusted).expect("trust");

        assert!(store.is_trusted(&trusted));
        assert!(!store.is_trusted(&sibling), "/a/bc must not inherit /a/b");
        assert!(
            !store.is_trusted(&child),
            "a child directory does not inherit the parent's grant"
        );
    }

    #[test]
    fn a_missing_store_trusts_nothing() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = TrustStore::load_from(&store_path(directory.path()));
        assert!(store.is_empty());
        assert!(!store.is_trusted(directory.path()));
    }

    #[test]
    fn a_malformed_store_trusts_nothing_and_never_panics() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = store_path(directory.path());

        fs::write(&path, "this is not toml [").expect("write");
        assert!(TrustStore::load_from(&path).is_empty());

        fs::write(&path, "version = 1\nprojects = 3\n").expect("write");
        assert!(TrustStore::load_from(&path).is_empty());

        fs::write(
            &path,
            "version = 99\n[projects.\"/tmp/x\"]\ntrusted_at = 1\n",
        )
        .expect("write");
        assert!(TrustStore::load_from(&path).is_empty());

        fs::write(&path, "").expect("write");
        assert!(TrustStore::load_from(&path).is_empty());
    }

    #[test]
    fn a_trust_call_merges_entries_written_meanwhile() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = store_path(directory.path());
        let first = workspace(directory.path(), "first");
        let second = workspace(directory.path(), "second");

        let mut writer = TrustStore::at(&path);
        writer.trust(&first).expect("first");

        let mut other = TrustStore::at(&path);
        other.trust(&second).expect("second");

        let reloaded = TrustStore::load_from(&path);
        assert!(reloaded.is_trusted(&first));
        assert!(reloaded.is_trusted(&second));
    }

    #[test]
    fn a_valid_empty_store_replaces_a_stale_in_memory_entry() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = store_path(directory.path());
        let workspace = workspace(directory.path(), "repo");

        let mut store = TrustStore::at(&path);
        store.trust(&workspace).expect("trust");
        assert!(store.is_trusted(&workspace));

        // Another process removed every entry and left a valid empty store.
        fs::write(&path, "version = 1\n").expect("write");
        assert!(!store.untrust(&workspace).expect("untrust"));
        assert!(!store.is_trusted(&workspace));
        assert!(!TrustStore::load_from(&path).is_trusted(&workspace));
    }

    #[test]
    fn a_store_path_is_beside_the_config_file() {
        assert_eq!(TrustStore::default_path(), config_home().join(STORE_FILE));
        assert_eq!(STORE_FILE, "trusted_workspaces.toml");
    }
}
