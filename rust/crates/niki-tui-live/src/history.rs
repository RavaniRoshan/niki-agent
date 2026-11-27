//! The prompts submitted this session and the file they outlive it in.

use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Prompt history: newest last, walked backwards from the composer.
///
/// The file holds one JSON string per line. A line that does not parse is
/// skipped, so a half-written file costs one entry rather than the rest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History {
    entries: Vec<String>,
    cursor: Option<usize>,
    draft: String,
    path: Option<PathBuf>,
}

impl History {
    /// Open the history at `path`, reading what earlier launches recorded.
    ///
    /// A missing file is an empty history; nothing is written until a prompt
    /// is recorded.
    #[must_use]
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            entries: load(&path),
            cursor: None,
            draft: String::new(),
            path: Some(path),
        }
    }

    /// The history at `$NIKI_HOME/history.jsonl`, or under `$HOME/.niki` when
    /// `NIKI_HOME` is unset.
    ///
    /// With neither variable set the history stays in memory, so a launch with
    /// no home directory still recalls what it was told.
    #[must_use]
    pub fn from_env() -> Self {
        match history_path(env::var_os("NIKI_HOME"), env::var_os("HOME")) {
            Some(path) => Self::open(path),
            None => Self::default(),
        }
    }

    /// The file this history is read from and appended to, when it has one.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Every recorded prompt, newest last.
    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Whether the arrows are walking back through recorded prompts.
    #[must_use]
    pub fn is_browsing(&self) -> bool {
        self.cursor.is_some()
    }

    /// Record a prompt that was sent, and say whether it was kept.
    ///
    /// A blank prompt and a repeat of the newest entry are not kept. A file
    /// that cannot be written costs the stored copy only, not the session.
    pub fn record(&mut self, prompt: &str) -> bool {
        self.reset();
        if prompt.trim().is_empty() || self.entries.last().is_some_and(|last| last == prompt) {
            return false;
        }
        self.entries.push(prompt.to_owned());
        self.append(prompt);
        true
    }

    /// Step one prompt back. The first step keeps `current` for [`Self::newer`].
    pub fn older(&mut self, current: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }
        match self.cursor {
            None => {
                self.draft = current.to_owned();
                self.cursor = Some(self.entries.len() - 1);
            }
            Some(0) => {}
            Some(index) => self.cursor = Some(index - 1),
        }
        self.cursor.map(|index| self.entries[index].clone())
    }

    /// Step one prompt forward, then back to the draft that was there first.
    pub fn newer(&mut self) -> Option<String> {
        match self.cursor {
            Some(index) if index + 1 < self.entries.len() => {
                self.cursor = Some(index + 1);
                Some(self.entries[index + 1].clone())
            }
            Some(_) => {
                self.cursor = None;
                Some(std::mem::take(&mut self.draft))
            }
            None => None,
        }
    }

    /// Forget the walk, so the next arrow starts from the newest prompt again.
    pub fn reset(&mut self) {
        self.cursor = None;
        self.draft.clear();
    }

    fn append(&self, prompt: &str) {
        let Some(path) = &self.path else {
            return;
        };
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && fs::create_dir_all(parent).is_err() {
                return;
            }
        }
        let Ok(line) = serde_json::to_string(prompt) else {
            return;
        };
        let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
            return;
        };
        let _ = writeln!(file, "{line}");
    }
}

fn load(path: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<String>(line).ok())
        .collect()
}

fn history_path(niki_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let root = non_empty(niki_home)
        .map(PathBuf::from)
        .or_else(|| non_empty(home).map(|home| PathBuf::from(home).join(".niki")))?;
    Some(root.join("history.jsonl"))
}

fn non_empty(value: Option<OsString>) -> Option<OsString> {
    value.filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_home_variable_names_the_file_and_home_is_the_fallback() {
        assert_eq!(
            history_path(Some("/profile".into()), Some("/home/person".into())),
            Some(PathBuf::from("/profile/history.jsonl"))
        );
        assert_eq!(
            history_path(None, Some("/home/person".into())),
            Some(PathBuf::from("/home/person/.niki/history.jsonl"))
        );
        assert_eq!(history_path(None, None), None);
        assert_eq!(
            history_path(Some("".into()), Some("/home/person".into())),
            Some(PathBuf::from("/home/person/.niki/history.jsonl"))
        );
    }
}
