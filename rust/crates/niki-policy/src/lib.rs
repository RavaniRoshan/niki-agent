//! Approval modes, shell allow-lists, and the workspace path boundary.
//!
//! The Python product gates tool actions with a live Manual/Auto/YOLO mode and
//! a shell allow-list. This crate is the policy half of that gate: it decides
//! whether a command runs, asks, or is refused, and it refuses any path that
//! resolves outside the workspace root. Refusing is deliberate — RULES 7 says a
//! path that escapes is refused, never clamped.

#![forbid(unsafe_code)]

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

/// The live approval mode behind Shift+Tab.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    /// Ask for every gated action that is not allow-listed.
    #[default]
    Manual,
    /// Allow allow-listed actions; ask for the rest.
    Auto,
    /// Allow every action the policy does not refuse.
    Yolo,
}

impl Mode {
    /// The next mode in the status-bar cycle: Manual → Auto → YOLO → Manual.
    #[must_use]
    pub fn cycle(self) -> Self {
        match self {
            Self::Manual => Self::Auto,
            Self::Auto => Self::Yolo,
            Self::Yolo => Self::Manual,
        }
    }

    /// The lowercase word the status bar shows for this mode.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Auto => "auto",
            Self::Yolo => "yolo",
        }
    }
}

/// Patterns a shell command must match to run unchallenged.
///
/// Parsed from comma- or newline-separated entries. An entry ending in `*` is a
/// prefix match on the whole command; any other entry matches the command
/// exactly. The entry `all` (any case) disables the check. An empty list
/// permits nothing, so every gated command is asked about.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AllowList {
    all: bool,
    patterns: Vec<String>,
}

impl AllowList {
    /// Parse a comma- or newline-separated allow-list.
    #[must_use]
    pub fn parse(spec: &str) -> Self {
        let mut all = false;
        let mut patterns = Vec::new();
        for entry in spec.split([',', '\n']) {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            if entry.eq_ignore_ascii_case("all") {
                all = true;
            } else {
                patterns.push(entry.to_owned());
            }
        }
        Self { all, patterns }
    }

    /// Whether `command` is allow-listed.
    #[must_use]
    pub fn permits(&self, command: &str) -> bool {
        if self.all {
            return true;
        }
        let command = command.trim();
        if command.is_empty() {
            return false;
        }
        self.patterns.iter().any(|pattern| {
            pattern
                .strip_suffix('*')
                .map_or_else(|| command == pattern, |prefix| command.starts_with(prefix))
        })
    }

    /// Whether the list carries the `all` entry.
    #[must_use]
    pub fn is_all(&self) -> bool {
        self.all
    }
}

impl From<&str> for AllowList {
    fn from(spec: &str) -> Self {
        Self::parse(spec)
    }
}

/// What the policy says about one gated command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Run it.
    Allow,
    /// Ask the user.
    Ask,
    /// Refuse it.
    Deny,
}

/// Decide a gated command for `mode`.
///
/// Manual asks unless the allow-list permits the command; Auto allows a
/// permit and asks otherwise; YOLO allows everything the policy does not deny.
/// A policy denial is always [`Decision::Deny`] and no mode can upgrade it —
/// here that is the blank command, which has nothing to run.
#[must_use]
pub fn decide(mode: Mode, allow: &AllowList, command: &str) -> Decision {
    if command.trim().is_empty() {
        return Decision::Deny;
    }
    if mode == Mode::Yolo || allow.permits(command) {
        Decision::Allow
    } else {
        Decision::Ask
    }
}

/// Resolve `candidate` and refuse it when it lands outside `root`.
///
/// Both paths are canonicalised, so `..` and symlinks are resolved before the
/// boundary check. A path that does not exist yet is resolved through its
/// nearest existing ancestor, which keeps file creation usable while still
/// refusing a parent that is a symlink out of the root. A dangling symlink
/// cannot be resolved and is refused.
///
/// # Errors
///
/// Returns [`PolicyError::Outside`] when the resolved path leaves the root,
/// and a [`PolicyError`] resolve/read variant when a path cannot be resolved
/// at all.
pub fn ensure_within(root: &Path, candidate: &Path) -> Result<PathBuf, PolicyError> {
    let root = root.canonicalize().map_err(|source| PolicyError::Root {
        path: root.to_path_buf(),
        source,
    })?;
    let candidate = absolute(candidate)?;
    let resolved = resolve(&candidate)?;
    if resolved == root || resolved.starts_with(&root) {
        Ok(resolved)
    } else {
        Err(PolicyError::Outside {
            root,
            candidate: resolved,
        })
    }
}

fn absolute(candidate: &Path) -> Result<PathBuf, PolicyError> {
    if candidate.is_absolute() {
        return Ok(candidate.to_path_buf());
    }
    let cwd = std::env::current_dir().map_err(|source| PolicyError::Resolve {
        path: candidate.to_path_buf(),
        source,
    })?;
    Ok(cwd.join(candidate))
}

fn resolve(candidate: &Path) -> Result<PathBuf, PolicyError> {
    let mut missing: Vec<&OsStr> = Vec::new();
    let mut cursor = candidate;
    loop {
        match cursor.canonicalize() {
            Ok(mut resolved) => {
                for name in missing.iter().rev() {
                    resolved.push(name);
                }
                return Ok(resolved);
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                if cursor.symlink_metadata().is_ok() {
                    // The entry exists but its link target does not. Treating
                    // it as "not created yet" would let a write follow the
                    // link out of the root, so refuse instead.
                    return Err(PolicyError::Unresolved {
                        path: cursor.to_path_buf(),
                    });
                }
                let Some(name) = cursor.file_name() else {
                    return Err(PolicyError::Unresolved {
                        path: candidate.to_path_buf(),
                    });
                };
                match cursor.parent() {
                    Some(parent) => {
                        missing.push(name);
                        cursor = parent;
                    }
                    None => {
                        return Err(PolicyError::Unresolved {
                            path: candidate.to_path_buf(),
                        });
                    }
                }
            }
            Err(source) => {
                return Err(PolicyError::Resolve {
                    path: cursor.to_path_buf(),
                    source,
                });
            }
        }
    }
}

/// Why a path is not usable.
#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    /// The workspace root itself cannot be resolved.
    #[error("cannot resolve the workspace root {path}: {source}")]
    Root {
        /// The configured root.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// A path component cannot be resolved.
    #[error("cannot resolve {path}: {source}")]
    Resolve {
        /// The component that failed.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// A path exists only as a link whose target cannot be resolved.
    #[error("cannot resolve {path}")]
    Unresolved {
        /// The path that could not be resolved.
        path: PathBuf,
    },
    /// The resolved path is outside the workspace root.
    #[error("{candidate} is outside the workspace root {root}")]
    Outside {
        /// The canonical workspace root.
        root: PathBuf,
        /// The resolved candidate that escaped.
        candidate: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        base: PathBuf,
    }

    impl Fixture {
        fn new(tag: &str) -> io::Result<Self> {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos());
            let base = std::env::temp_dir().join(format!(
                "niki-policy-{tag}-{}-{nanos}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&base)?;
            Ok(Self { base })
        }

        fn root(&self) -> PathBuf {
            self.base.join("root")
        }

        fn outside(&self) -> PathBuf {
            self.base.join("outside")
        }

        fn make_root(&self) -> io::Result<()> {
            std::fs::create_dir_all(self.root())
        }

        fn make_outside(&self) -> io::Result<()> {
            std::fs::create_dir_all(self.outside())
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn cycle_wraps_in_status_bar_order() {
        assert_eq!(Mode::Manual.cycle(), Mode::Auto);
        assert_eq!(Mode::Auto.cycle(), Mode::Yolo);
        assert_eq!(Mode::Yolo.cycle(), Mode::Manual);
    }

    #[test]
    fn labels_are_the_lowercase_status_words() {
        assert_eq!(Mode::Manual.label(), "manual");
        assert_eq!(Mode::Auto.label(), "auto");
        assert_eq!(Mode::Yolo.label(), "yolo");
        assert_eq!(Mode::default(), Mode::Manual);
    }

    #[test]
    fn suffix_star_is_a_prefix_match_and_other_entries_are_exact() {
        let allow = AllowList::parse("git*, ls");
        assert!(allow.permits("git status"));
        assert!(allow.permits("git"));
        assert!(allow.permits("ls"));
        assert!(!allow.permits("ls -la"));
        assert!(!allow.permits("cargo test"));

        let star = AllowList::parse("*");
        assert!(star.permits("anything at all"));
        assert!(!star.permits(""));
    }

    #[test]
    fn comma_and_newline_both_separate_entries() {
        let allow = AllowList::parse("npm test\ncargo build,ls , ,\n");
        assert!(allow.permits("npm test"));
        assert!(allow.permits("cargo build"));
        assert!(allow.permits("ls"));
        assert!(!allow.permits(""));
    }

    #[test]
    fn all_disables_the_check_in_any_case() {
        for spec in ["all", "ALL", " all "] {
            let allow = AllowList::parse(spec);
            assert!(allow.is_all(), "{spec}");
            assert!(allow.permits("rm -rf /"));
        }
    }

    #[test]
    fn an_empty_list_denies_every_gated_command() {
        assert!(!AllowList::default().permits("ls"));
        assert!(!AllowList::parse("").permits("ls"));
        assert!(!AllowList::parse(" , \n ").permits("ls"));
    }

    #[test]
    fn a_path_inside_the_root_resolves() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new("inside")?;
        fixture.make_root()?;
        let file = fixture.root().join("sub").join("file.txt");
        std::fs::create_dir_all(file.parent().ok_or("no parent")?)?;
        std::fs::write(&file, "hi")?;

        let resolved = ensure_within(&fixture.root(), &file)?;
        assert_eq!(resolved, file.canonicalize()?);
        Ok(())
    }

    #[test]
    fn a_missing_file_inside_the_root_resolves_through_its_parent()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new("new-file")?;
        fixture.make_root()?;
        let file = fixture.root().join("created-later.txt");

        let resolved = ensure_within(&fixture.root(), &file)?;
        assert_eq!(
            resolved,
            fixture.root().canonicalize()?.join("created-later.txt")
        );
        Ok(())
    }

    #[test]
    fn parent_traversal_out_of_the_root_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new("traversal")?;
        fixture.make_root()?;
        fixture.make_outside()?;
        std::fs::write(fixture.outside().join("secret.txt"), "secret")?;

        let candidate = fixture.root().join("..").join("outside").join("secret.txt");
        let error = ensure_within(&fixture.root(), &candidate).err();
        assert!(
            matches!(error, Some(PolicyError::Outside { .. })),
            "{error:?}"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_that_escapes_the_root_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new("symlink")?;
        fixture.make_root()?;
        fixture.make_outside()?;
        std::fs::write(fixture.outside().join("secret.txt"), "secret")?;
        symlink(fixture.outside(), fixture.root().join("escape"))?;

        let existing = fixture.root().join("escape").join("secret.txt");
        let error = ensure_within(&fixture.root(), &existing).err();
        assert!(
            matches!(error, Some(PolicyError::Outside { .. })),
            "{error:?}"
        );

        let missing = fixture.root().join("escape").join("new.txt");
        let error = ensure_within(&fixture.root(), &missing).err();
        assert!(
            matches!(error, Some(PolicyError::Outside { .. })),
            "{error:?}"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_that_stays_inside_the_root_is_allowed() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new("symlink-inside")?;
        fixture.make_root()?;
        let target = fixture.root().join("target");
        std::fs::create_dir_all(&target)?;
        let file = target.join("file.txt");
        std::fs::write(&file, "hi")?;
        symlink(&target, fixture.root().join("link"))?;

        let resolved = ensure_within(&fixture.root(), &fixture.root().join("link/file.txt"))?;
        assert_eq!(resolved, file.canonicalize()?);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new("dangling")?;
        fixture.make_root()?;
        let dangling = fixture.root().join("dangling");
        symlink(fixture.outside().join("never-created"), &dangling)?;

        let error = ensure_within(&fixture.root(), &dangling).err();
        assert!(
            matches!(error, Some(PolicyError::Unresolved { .. })),
            "{error:?}"
        );
        Ok(())
    }

    #[test]
    fn decide_matches_the_mode_truth_table() {
        let empty = AllowList::default();
        let listed = AllowList::parse("ls");
        let everything = AllowList::parse("all");

        assert_eq!(decide(Mode::Manual, &empty, "ls"), Decision::Ask);
        assert_eq!(decide(Mode::Manual, &listed, "ls"), Decision::Allow);
        assert_eq!(decide(Mode::Manual, &listed, "ls -la"), Decision::Ask);

        assert_eq!(decide(Mode::Auto, &empty, "ls"), Decision::Ask);
        assert_eq!(decide(Mode::Auto, &listed, "ls"), Decision::Allow);
        assert_eq!(decide(Mode::Auto, &listed, "rm -rf /"), Decision::Ask);
        assert_eq!(decide(Mode::Auto, &everything, "rm -rf /"), Decision::Allow);

        assert_eq!(decide(Mode::Yolo, &empty, "rm -rf /"), Decision::Allow);

        assert_eq!(decide(Mode::Manual, &empty, ""), Decision::Deny);
        assert_eq!(decide(Mode::Auto, &listed, "   "), Decision::Deny);
        assert_eq!(decide(Mode::Yolo, &everything, "\n"), Decision::Deny);
    }
}
