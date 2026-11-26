//! The subset of `.gitignore` the file index honors.
//!
//! Supported: blank lines, `#` comments, glob lines, a leading `/` that anchors
//! a pattern to the workspace root, and a trailing `/` that makes an entry
//! match directories only. A pattern without a `/` matches at any depth, as it
//! does in git.
//!
//! Not supported, stated here so the limit is visible rather than surprising:
//! `!` negation (the line is dropped, so the ignore stays in force), `\`
//! escapes, character classes beyond what `globset` accepts, nested
//! `.gitignore` files (only the root file is read), and `.git/info/exclude`.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

/// Directory names that are never indexed, whatever `.gitignore` says.
pub(crate) const SKIPPED_DIRS: [&str; 7] = [
    ".git",
    "node_modules",
    "target",
    ".venv",
    "__pycache__",
    "dist",
    "build",
];

#[derive(Debug, Default)]
pub(crate) struct Rules {
    patterns: GlobSet,
    directory_patterns: GlobSet,
}

impl Rules {
    /// Read the root `.gitignore`. A missing or unreadable file means no rules.
    pub(crate) fn load(root: &Path) -> Self {
        fs::read_to_string(root.join(".gitignore"))
            .map_or_else(|_| Self::default(), |text| Self::parse(&text))
    }

    fn parse(text: &str) -> Self {
        let mut patterns = GlobSetBuilder::new();
        let mut directory_patterns = GlobSetBuilder::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                continue;
            }
            let (pattern, directory) = line
                .strip_suffix('/')
                .map_or((line, false), |stripped| (stripped, true));
            let anchored = pattern.starts_with('/') || pattern.contains('/');
            let pattern = pattern.trim_start_matches('/');
            if pattern.is_empty() {
                continue;
            }
            let expanded = if anchored {
                pattern.to_owned()
            } else {
                format!("**/{pattern}")
            };
            let Ok(glob) = GlobBuilder::new(expanded.as_str())
                .literal_separator(true)
                .build()
            else {
                continue;
            };
            if directory {
                directory_patterns.add(glob);
            } else {
                patterns.add(glob);
            }
        }
        Self {
            patterns: patterns.build().unwrap_or_else(|_| GlobSet::empty()),
            directory_patterns: directory_patterns
                .build()
                .unwrap_or_else(|_| GlobSet::empty()),
        }
    }

    /// Whether a directory, named relative to the root, is pruned from the walk.
    pub(crate) fn skip_dir(&self, relative: &Path) -> bool {
        let built_in = relative
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| SKIPPED_DIRS.contains(&name));
        built_in || self.patterns.is_match(relative) || self.directory_patterns.is_match(relative)
    }

    /// Whether a file, named relative to the root, is left out of the index.
    pub(crate) fn skip_file(&self, relative: &Path) -> bool {
        // A `.git` file marks a worktree, where `.git` is not a directory.
        let git_marker = relative.file_name().and_then(OsStr::to_str) == Some(".git");
        git_marker || self.patterns.is_match(relative)
    }
}
