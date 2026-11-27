//! Running the person's own editor on the composer.
//!
//! The editor is resolved in this order, taking the first value that names a
//! program:
//!
//! 1. `$VISUAL`
//! 2. `$EDITOR`
//! 3. `vi`, or `notepad` on Windows
//!
//! A value may carry arguments — `code --wait`, `vim -f` — which are split on
//! whitespace and passed to the program directly. Nothing here goes through a
//! shell, so quoting and shell syntax in the value do not mean anything.
//!
//! The spawn and the read back are separate from the terminal: [`edit_file`]
//! takes the program and the path, so a test can drive it with a script.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{SystemTime, UNIX_EPOCH};

/// Largest edited prompt read back, in bytes.
///
/// A prompt is text a person typed, so a megabyte is far past anything real; a
/// larger file is a mistake — an editor writing something else, or a binary —
/// and is reported rather than cut down to the limit, because a truncated
/// prompt would be sent without the person seeing what was lost.
pub const MAX_EDITED_BYTES: u64 = 1024 * 1024;

/// The editor used when neither variable names one.
pub const FALLBACK_EDITOR: &str = if cfg!(windows) { "notepad" } else { "vi" };

/// One editor invocation: a program and the arguments that precede the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCommand {
    program: String,
    args: Vec<String>,
}

impl EditorCommand {
    /// A program and its arguments.
    #[must_use]
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }

    /// Split a `$VISUAL` or `$EDITOR` value on whitespace.
    ///
    /// Returns `None` for a value with no program in it, so an empty variable
    /// falls through to the next choice instead of naming nothing.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let mut parts = value.split_whitespace();
        let program = parts.next()?;
        Some(Self {
            program: program.to_owned(),
            args: parts.map(str::to_owned).collect(),
        })
    }

    /// The program to start.
    #[must_use]
    pub fn program(&self) -> &str {
        &self.program
    }

    /// The arguments passed between the program and the file path.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }
}

/// Resolve the editor from the two variables' values.
///
/// `$VISUAL` comes first because it names a full-screen editor, which is what a
/// prompt needs; `$EDITOR` is the older, line-oriented convention and is the
/// fallback rather than the first choice.
#[must_use]
pub fn resolve_editor(visual: Option<&str>, editor: Option<&str>) -> EditorCommand {
    visual
        .and_then(EditorCommand::parse)
        .or_else(|| editor.and_then(EditorCommand::parse))
        .unwrap_or_else(|| EditorCommand::new(FALLBACK_EDITOR, Vec::new()))
}

/// Resolve the editor from this process's environment.
#[must_use]
pub fn resolve_editor_from_env() -> EditorCommand {
    resolve_editor(
        std::env::var("VISUAL").ok().as_deref(),
        std::env::var("EDITOR").ok().as_deref(),
    )
}

/// Why the editor hand-off did not produce a new draft.
#[derive(Debug)]
pub enum EditError {
    /// The program could not be started at all.
    Spawn { program: String, message: String },
    /// The editor ran and returned a failure status; `None` means a signal.
    Status(Option<i32>),
    /// The file is larger than [`MAX_EDITED_BYTES`].
    TooLarge { bytes: u64 },
    /// The file is not UTF-8 text.
    NotUtf8,
    /// The file could not be read back.
    Read(io::Error),
}

impl EditError {
    /// The transcript row a person reads after a failed edit.
    #[must_use]
    pub fn warning(&self) -> String {
        match self {
            Self::Spawn { program, message } => format!("could not start {program}: {message}"),
            Self::Status(Some(code)) => format!("editor exited with status {code}"),
            Self::Status(None) => "editor exited on a signal".to_owned(),
            Self::TooLarge { bytes } => format!(
                "the edited prompt is {bytes} bytes, over the {MAX_EDITED_BYTES} byte limit; \
                 kept the previous prompt"
            ),
            Self::NotUtf8 => "the edited prompt is not UTF-8; kept the previous prompt".to_owned(),
            Self::Read(error) => format!("could not read the edited prompt: {error}"),
        }
    }
}

/// Run `command` on `path` and wait for it.
///
/// # Errors
///
/// [`EditError::Spawn`] when the program cannot be started, and
/// [`EditError::Status`] when it starts but exits unsuccessfully.
pub fn run_editor(command: &EditorCommand, path: &Path) -> Result<(), EditError> {
    let status = ProcessCommand::new(command.program())
        .args(command.args())
        .arg(path)
        .status()
        .map_err(|error| EditError::Spawn {
            program: command.program().to_owned(),
            message: error.to_string(),
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(EditError::Status(status.code()))
    }
}

/// Read the edited file back.
///
/// The length is checked before reading so the warning can name the real size;
/// the read itself is bounded so a file that grows while it is read still
/// cannot exceed the limit in memory.
///
/// # Errors
///
/// [`EditError::TooLarge`] past [`MAX_EDITED_BYTES`], [`EditError::NotUtf8`] for
/// binary content, and [`EditError::Read`] when the file cannot be opened or
/// read.
pub fn read_edited(path: &Path) -> Result<String, EditError> {
    let file = File::open(path).map_err(EditError::Read)?;
    let length = file.metadata().map_err(EditError::Read)?.len();
    if length > MAX_EDITED_BYTES {
        return Err(EditError::TooLarge { bytes: length });
    }
    let mut bytes = Vec::new();
    file.take(MAX_EDITED_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(EditError::Read)?;
    if bytes.len() as u64 > MAX_EDITED_BYTES {
        return Err(EditError::TooLarge {
            bytes: bytes.len() as u64,
        });
    }
    String::from_utf8(bytes).map_err(|_| EditError::NotUtf8)
}

/// Run the editor and read back what it left, minus the final line break.
///
/// # Errors
///
/// Any [`EditError`] from [`run_editor`] or [`read_edited`].
pub fn edit_file(command: &EditorCommand, path: &Path) -> Result<String, EditError> {
    run_editor(command, path)?;
    let text = read_edited(path)?;
    Ok(without_trailing_newline(&text).to_owned())
}

/// Drop the one line break an editor leaves at the end of the file.
///
/// Only one: a prompt that deliberately ends with a blank line keeps it, and a
/// file ending with two newlines loses exactly one. A bare carriage return is
/// text, not a line break.
#[must_use]
pub fn without_trailing_newline(text: &str) -> &str {
    match text.strip_suffix('\n') {
        Some(rest) => rest.strip_suffix('\r').unwrap_or(rest),
        None => text,
    }
}

/// The file the editor works on, removed when this value is dropped.
#[derive(Debug)]
pub struct TempPrompt {
    path: PathBuf,
}

impl TempPrompt {
    /// Write `draft` to a new `.md` file in the OS temporary directory.
    ///
    /// The file is created exclusively and, on Unix, readable only by its
    /// owner: a prompt can hold a path or a secret that is not meant to be
    /// world-readable in `/tmp`.
    ///
    /// # Errors
    ///
    /// The failure to create or write the file; the caller keeps the draft on
    /// screen.
    pub fn create(draft: &str) -> io::Result<Self> {
        let directory = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        for attempt in 0..16 {
            let path = directory.join(format!(
                "niki-prompt-{}-{stamp}-{attempt}.md",
                std::process::id()
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(mut file) => {
                    if let Err(error) = file.write_all(draft.as_bytes()) {
                        let _ = fs::remove_file(&path);
                        return Err(error);
                    }
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not find a free temporary file name",
        ))
    }

    /// The path passed to the editor.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempPrompt {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn script(body: &str) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("a temporary directory");
        let path = directory.path().join("editor.sh");
        fs::write(&path, body).expect("write the script");
        let mut permissions = fs::metadata(&path).expect("script metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).expect("make the script executable");
        (directory, path)
    }

    #[test]
    fn visual_wins_over_editor_and_both_split_their_arguments() {
        let visual = resolve_editor(Some("code --wait"), Some("vim -f"));
        assert_eq!(visual.program(), "code");
        assert_eq!(visual.args(), ["--wait"]);

        let editor = resolve_editor(None, Some("  vim\t-f "));
        assert_eq!(editor.program(), "vim");
        assert_eq!(editor.args(), ["-f"]);
    }

    #[test]
    fn an_empty_variable_falls_through_to_the_next_choice() {
        assert_eq!(resolve_editor(Some("   "), Some("nano")).program(), "nano");
        let fallback = resolve_editor(Some(""), None);
        assert_eq!(fallback.program(), FALLBACK_EDITOR);
        assert!(fallback.args().is_empty());
    }

    #[test]
    fn the_fallback_editor_is_vi_outside_windows() {
        #[cfg(not(windows))]
        assert_eq!(resolve_editor(None, None).program(), "vi");
        #[cfg(windows)]
        assert_eq!(resolve_editor(None, None).program(), "notepad");
    }

    #[test]
    fn one_trailing_newline_is_trimmed_and_only_one() {
        assert_eq!(without_trailing_newline("hello\n"), "hello");
        assert_eq!(without_trailing_newline("hello\r\n"), "hello");
        assert_eq!(without_trailing_newline("hello"), "hello");
        assert_eq!(without_trailing_newline("hello\n\n"), "hello\n");
        assert_eq!(
            without_trailing_newline("hello\r"),
            "hello\r",
            "a bare carriage return is text"
        );
        assert_eq!(without_trailing_newline(""), "");
    }

    #[test]
    fn the_temporary_prompt_holds_the_draft_and_is_removed_on_drop() {
        let draft = "# a draft\n\nwith a second paragraph\n";
        let path = {
            let file = TempPrompt::create(draft).expect("a temporary prompt");
            assert_eq!(
                file.path()
                    .extension()
                    .and_then(|extension| extension.to_str()),
                Some("md")
            );
            assert_eq!(
                file.path().parent(),
                Some(std::env::temp_dir().as_path()),
                "the file lives in the OS temporary directory"
            );
            assert_eq!(fs::read_to_string(file.path()).expect("the draft"), draft);
            file.path().to_owned()
        };
        assert!(!path.exists(), "the file is removed with the prompt");
    }

    #[cfg(unix)]
    #[test]
    fn an_editor_that_rewrites_the_file_loads_its_text() {
        let (_scripts, editor) = script("#!/bin/sh\nprintf 'the edited prompt\\n\\n' > \"$1\"\n");
        let working = tempfile::tempdir().expect("a working directory");
        let path = working.path().join("prompt.md");
        fs::write(&path, "the draft").expect("the draft");

        let command = EditorCommand::new(editor.display().to_string(), Vec::new());
        let text = edit_file(&command, &path).expect("the edited text");
        assert_eq!(text, "the edited prompt\n", "one newline is trimmed");
    }

    #[cfg(unix)]
    #[test]
    fn an_editor_that_fails_reports_its_status_and_leaves_the_file_alone() {
        let (_scripts, editor) = script("#!/bin/sh\nexit 3\n");
        let working = tempfile::tempdir().expect("a working directory");
        let path = working.path().join("prompt.md");
        fs::write(&path, "the draft").expect("the draft");

        let command = EditorCommand::new(editor.display().to_string(), Vec::new());
        let error = edit_file(&command, &path).expect_err("the exit status");
        assert!(matches!(error, EditError::Status(Some(3))), "{error:?}");
        assert_eq!(error.warning(), "editor exited with status 3");
    }

    #[test]
    fn a_missing_editor_is_a_spawn_error_naming_the_program() {
        let working = tempfile::tempdir().expect("a working directory");
        let path = working.path().join("prompt.md");
        fs::write(&path, "the draft").expect("the draft");
        let missing = working.path().join("no-such-editor");
        let program = missing.display().to_string();

        let command = EditorCommand::new(program.clone(), Vec::new());
        let error = edit_file(&command, &path).expect_err("the missing program");
        assert!(
            matches!(&error, EditError::Spawn { program: named, .. } if named == &program),
            "{error:?}"
        );
        let warning = error.warning();
        assert!(warning.starts_with("could not start "), "{warning}");
        assert!(warning.contains(&program), "{warning}");
    }

    #[test]
    fn a_file_at_the_limit_loads_and_a_larger_one_is_refused() {
        let working = tempfile::tempdir().expect("a working directory");
        let at_limit = working.path().join("at.md");
        fs::write(&at_limit, "a".repeat(MAX_EDITED_BYTES as usize)).expect("the file");
        assert_eq!(
            read_edited(&at_limit).expect("the limit is allowed").len() as u64,
            MAX_EDITED_BYTES
        );

        let over = working.path().join("over.md");
        fs::write(&over, "a".repeat(MAX_EDITED_BYTES as usize + 1)).expect("the file");
        let error = read_edited(&over).expect_err("the file is too large");
        assert!(
            matches!(error, EditError::TooLarge { bytes } if bytes == MAX_EDITED_BYTES + 1),
            "{error:?}"
        );
        assert!(error.warning().contains("kept the previous prompt"));
    }
}
