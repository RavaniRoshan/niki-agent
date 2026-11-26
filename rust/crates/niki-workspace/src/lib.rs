//! Workspace root detection and the file index that grep and glob read.
//!
//! The root is the boundary every file tool respects (rule 7 of `RULES.md`).
//! Index entries are built by joining a directory entry name onto a directory
//! this crate already enumerated, so no entry can name a path outside the root,
//! and symlinks are not followed for the same reason.
//!
//! The index is built on the first request and reused until [`Root::refresh`].
//!
//! Ignore rules are the subset of `.gitignore` described in `src/ignore.rs`:
//! glob lines, `#` comments, a leading `/` for root-anchored patterns, and a
//! trailing `/` for directory-only entries. Negation, escapes, and nested
//! `.gitignore` files are not honored, and only the root file is read.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use globset::GlobBuilder;
use regex::Regex;

mod ignore;

use ignore::Rules;

/// Walk up from `start` to the nearest directory holding `.git` or `.niki`.
///
/// A file path resolves from its parent directory. The returned path is
/// canonicalized when the filesystem allows it, so it compares equal to other
/// canonical paths. A `.git` file counts as a marker: that is how a git worktree
/// points at its real repository. A start path with no parent is searched as
/// itself and yields `None` unless it holds a marker.
#[must_use]
pub fn detect_root(start: impl AsRef<Path>) -> Option<PathBuf> {
    let start = start.as_ref();
    let start = if start.is_file() {
        start.parent()?
    } else {
        start
    };
    start
        .ancestors()
        .find(|directory| is_marker(directory))
        .map(|directory| fs::canonicalize(directory).unwrap_or_else(|_| directory.to_path_buf()))
}

fn is_marker(directory: &Path) -> bool {
    directory.join(".git").exists() || directory.join(".niki").is_dir()
}

/// A workspace root plus its lazily built file index.
#[derive(Debug)]
pub struct Root {
    root: PathBuf,
    rules: Rules,
    index: Mutex<Option<Arc<Vec<PathBuf>>>>,
}

impl Root {
    /// Detect the root that contains `start`.
    #[must_use]
    pub fn detect(start: impl AsRef<Path>) -> Option<Self> {
        detect_root(start).map(Self::at)
    }

    fn at(root: PathBuf) -> Self {
        let rules = Rules::load(&root);
        Self {
            root,
            rules,
            index: Mutex::new(None),
        }
    }

    /// The root directory, as returned by [`detect_root`].
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// Indexed files, sorted and named relative to the root.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::Io`] when the root cannot be read.
    pub fn files(&self) -> Result<Vec<PathBuf>, WorkspaceError> {
        Ok(self.snapshot()?.as_ref().clone())
    }

    /// Rescan the root and replace the index, returning the file count.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::Io`] when the root cannot be read.
    pub fn refresh(&self) -> Result<usize, WorkspaceError> {
        let files = Arc::new(self.scan()?);
        let count = files.len();
        *self.lock() = Some(files);
        Ok(count)
    }

    /// Lines of indexed files matching `pattern`, at most `limit` of them.
    ///
    /// Files that are not UTF-8 text or cannot be read are skipped: one binary
    /// file should not fail a search. A limit of zero searches nothing.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::Regex`] for an invalid pattern and
    /// [`WorkspaceError::Io`] when the root cannot be read.
    pub fn grep(&self, pattern: &str, limit: usize) -> Result<Vec<LineMatch>, WorkspaceError> {
        let regex = Regex::new(pattern)?;
        let files = self.snapshot()?;
        let mut matches = Vec::new();
        for relative in files.iter() {
            if matches.len() >= limit {
                break;
            }
            let Ok(bytes) = fs::read(self.root.join(relative)) else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            for (number, line) in text.lines().enumerate() {
                if regex.is_match(line) {
                    matches.push(LineMatch {
                        path: relative.clone(),
                        line: number + 1,
                        text: line.to_owned(),
                    });
                    if matches.len() >= limit {
                        break;
                    }
                }
            }
        }
        Ok(matches)
    }

    /// Indexed files matching `pattern`, at most `limit` of them.
    ///
    /// The pattern is matched against the root-relative path, and `*` does not
    /// cross a directory separator: `*.rs` matches only files at the root,
    /// `**/*.rs` matches at any depth. Results keep the index order.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError::Glob`] for an invalid pattern and
    /// [`WorkspaceError::Io`] when the root cannot be read.
    pub fn glob(&self, pattern: &str, limit: usize) -> Result<Vec<PathBuf>, WorkspaceError> {
        let matcher = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()?
            .compile_matcher();
        let files = self.snapshot()?;
        let found = files
            .iter()
            .filter(|relative| matcher.is_match(relative))
            .take(limit)
            .cloned()
            .collect();
        Ok(found)
    }

    fn snapshot(&self) -> Result<Arc<Vec<PathBuf>>, WorkspaceError> {
        {
            let index = self.lock();
            if let Some(files) = index.as_ref() {
                return Ok(Arc::clone(files));
            }
        }
        let scanned = Arc::new(self.scan()?);
        let mut index = self.lock();
        Ok(Arc::clone(index.get_or_insert(scanned)))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Arc<Vec<PathBuf>>>> {
        // The guard is only ever held for one assignment or clone, so a
        // poisoned lock still holds a usable index.
        self.index.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn scan(&self) -> Result<Vec<PathBuf>, WorkspaceError> {
        let mut pending = vec![PathBuf::new()];
        let mut files = Vec::new();
        while let Some(directory) = pending.pop() {
            let absolute = if directory.as_os_str().is_empty() {
                self.root.clone()
            } else {
                self.root.join(&directory)
            };
            let entries = match fs::read_dir(&absolute) {
                Ok(entries) => entries,
                Err(source) if directory.as_os_str().is_empty() => {
                    return Err(WorkspaceError::Io {
                        path: absolute,
                        source,
                    });
                }
                // An unreadable subdirectory is pruned rather than failing the
                // whole index, which is what a search tool should do.
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                let relative = directory.join(entry.file_name());
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() {
                    if !self.rules.skip_dir(&relative) {
                        pending.push(relative);
                    }
                } else if file_type.is_file() && !self.rules.skip_file(&relative) {
                    files.push(relative);
                }
            }
        }
        files.sort();
        Ok(files)
    }
}

/// One matching line of one indexed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMatch {
    /// Path relative to the workspace root.
    pub path: PathBuf,
    /// One-based line number.
    pub line: usize,
    /// The line without its terminator.
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("cannot read {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid regular expression: {0}")]
    Regex(#[from] regex::Error),
    #[error("invalid glob pattern: {0}")]
    Glob(#[from] globset::Error),
}
