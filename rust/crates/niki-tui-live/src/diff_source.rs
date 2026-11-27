//! The workspace's working tree as unified diff text.
//!
//! Staged and unstaged changes come from `git diff`, untracked files are read
//! and turned into an all-added diff, and every file is handed to
//! [`parse_unified`] and [`render_diff`] so the screen and the headless mode
//! draw the same thing. Paths are arguments, never a shell command, and a
//! missing `git` or a directory outside a repository is a typed error the caller
//! prints on one line.
//!
//! Binary files produce a one-line note instead of their content. Output is
//! capped per section by [`MAX_FILES`] and [`MAX_LINES`]; the caller reports how
//! much was withheld.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use niki_render::{DiffKind, DiffTheme, FileDiff, RenderedLine, parse_unified, render_diff};

/// The width a diff is rendered at before the screen clips it to its own.
pub const RENDER_WIDTH: usize = 200;
/// Files one section shows at most.
pub const MAX_FILES: usize = 20;
/// Rendered lines one section shows at most.
pub const MAX_LINES: usize = 400;
/// The largest untracked file that is read to build its diff.
const MAX_UNTRACKED_BYTES: u64 = 1 << 20;

/// Which changes a `/diff` call shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffSection {
    /// Changes in the index.
    Staged,
    /// Changes in the working tree that are not staged.
    Unstaged,
    /// Files git does not track yet.
    Untracked,
}

impl DiffSection {
    /// Every section, in the order `/diff` prints them.
    pub const ALL: [Self; 3] = [Self::Staged, Self::Unstaged, Self::Untracked];

    /// The name used as the section heading and as the `/diff` argument.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Staged => "staged",
            Self::Unstaged => "unstaged",
            Self::Untracked => "untracked",
        }
    }

    /// The section an argument names, ignoring case.
    #[must_use]
    pub fn from_argument(argument: &str) -> Option<Self> {
        let wanted = argument.trim().to_ascii_lowercase();
        Self::ALL
            .into_iter()
            .find(|section| section.name() == wanted.as_str())
    }
}

/// One changed file, already parsed and rendered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// The path both sides name, without the `a/`/`b/` prefix.
    pub path: String,
    /// The rendered diff, empty when the file is reported by a note.
    pub rendered: Vec<RenderedLine>,
    /// Why the file has no rendered lines, when it does not.
    pub note: Option<String>,
    /// Lines the file gained.
    pub additions: usize,
    /// Lines the file lost.
    pub deletions: usize,
}

/// One section of the working tree, capped for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionView {
    /// Which changes this section holds.
    pub section: DiffSection,
    /// How many files the section has, shown or not.
    pub file_count: usize,
    /// Lines every file gained, including withheld files.
    pub additions: usize,
    /// Lines every file lost, including withheld files.
    pub deletions: usize,
    /// The files that fit under the cap.
    pub files: Vec<FileChange>,
    /// Files the cap left out.
    pub withheld_files: usize,
    /// Rendered lines the cap left out.
    pub withheld_lines: usize,
}

impl SectionView {
    /// The section as plain lines: a heading, a path per file, its lines, the
    /// withheld count and the summary.
    #[must_use]
    pub fn text_lines(&self) -> Vec<String> {
        let mut lines = vec![self.section.name().to_owned()];
        for file in &self.files {
            match &file.note {
                Some(note) => lines.push(format!("{}: {note}", file.path)),
                None => {
                    lines.push(file.path.clone());
                    lines.extend(file.rendered.iter().map(RenderedLine::text));
                }
            }
        }
        if let Some(withheld) = withheld_line(self.withheld_files, self.withheld_lines) {
            lines.push(withheld);
        }
        lines.push(summary_line(
            self.file_count,
            self.additions,
            self.deletions,
        ));
        lines
    }
}

/// The section summary, e.g. `3 files, +42 −7`.
#[must_use]
pub fn summary_line(files: usize, additions: usize, deletions: usize) -> String {
    let noun = if files == 1 { "file" } else { "files" };
    format!("{files} {noun}, +{additions} −{deletions}")
}

/// What the cap left out, or `None` when it left out nothing.
#[must_use]
pub fn withheld_line(files: usize, lines: usize) -> Option<String> {
    if files == 0 && lines == 0 {
        return None;
    }
    let mut parts = Vec::new();
    if files > 0 {
        let noun = if files == 1 { "file" } else { "files" };
        parts.push(format!("{files} more {noun}"));
    }
    if lines > 0 {
        let noun = if lines == 1 { "line" } else { "lines" };
        parts.push(format!("{lines} more {noun}"));
    }
    Some(format!("{} withheld", parts.join(" and ")))
}

/// Every section rendered into plain text, or `no changes`.
#[must_use]
pub fn plain_text(views: &[SectionView]) -> String {
    if views.iter().all(|view| view.files.is_empty()) {
        return "no changes".to_owned();
    }
    let mut blocks = Vec::new();
    for view in views.iter().filter(|view| !view.files.is_empty()) {
        blocks.push(view.text_lines().join("\n"));
    }
    blocks.join("\n\n")
}

/// Why the working tree could not be read.
#[derive(Debug)]
pub enum DiffError {
    /// `git` is not on the path.
    GitMissing,
    /// The directory is not a work tree.
    NotARepository {
        /// The directory `/diff` ran in.
        root: PathBuf,
    },
    /// Git started but refused or failed.
    GitFailed {
        /// The subcommand that failed.
        command: &'static str,
        /// The first line of its error output.
        message: String,
    },
    /// The `git` process could not be started or talked to.
    Spawn {
        /// What the operating system said.
        message: String,
    },
}

impl fmt::Display for DiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GitMissing => write!(formatter, "git is not installed; /diff needs it"),
            Self::NotARepository { root } => {
                write!(formatter, "not a git repository: {}", root.display())
            }
            Self::GitFailed { command, message } => {
                write!(formatter, "git {command} failed: {message}")
            }
            Self::Spawn { message } => write!(formatter, "could not run git: {message}"),
        }
    }
}

impl Error for DiffError {}

/// Read the named sections of the working tree under `root`.
///
/// # Errors
///
/// Returns [`DiffError`] when git is missing, `root` is not a work tree, or git
/// fails. A file that cannot be read becomes a note on that file, not an error
/// for the section.
pub fn collect(
    root: &Path,
    sections: &[DiffSection],
    width: usize,
    theme: &DiffTheme,
) -> Result<Vec<SectionView>, DiffError> {
    collect_with("git", root, sections, width, theme)
}

/// The `git` program name is a parameter so the missing-git path is testable
/// without touching the process environment.
fn collect_with(
    program: &str,
    root: &Path,
    sections: &[DiffSection],
    width: usize,
    theme: &DiffTheme,
) -> Result<Vec<SectionView>, DiffError> {
    ensure_repository(program, root)?;
    let mut views = Vec::with_capacity(sections.len());
    for section in sections {
        let files = match section {
            DiffSection::Staged => diff_files(program, root, true, width, theme)?,
            DiffSection::Unstaged => diff_files(program, root, false, width, theme)?,
            DiffSection::Untracked => untracked_files(program, root, width, theme)?,
        };
        views.push(capped(*section, files));
    }
    Ok(views)
}

/// Run git with the path bound as an argument, never through a shell.
fn git(program: &str, root: &Path, arguments: &[&str]) -> Result<std::process::Output, DiffError> {
    Command::new(program)
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => DiffError::GitMissing,
            _ => DiffError::Spawn {
                message: one_line(&error.to_string()),
            },
        })
}

fn ensure_repository(program: &str, root: &Path) -> Result<(), DiffError> {
    let output = git(program, root, &["rev-parse", "--is-inside-work-tree"])?;
    let inside = String::from_utf8_lossy(&output.stdout);
    if output.status.success() && inside.trim() == "true" {
        return Ok(());
    }
    Err(DiffError::NotARepository {
        root: root.to_path_buf(),
    })
}

fn diff_files(
    program: &str,
    root: &Path,
    cached: bool,
    width: usize,
    theme: &DiffTheme,
) -> Result<Vec<FileChange>, DiffError> {
    let mut arguments = vec!["-c", "core.quotepath=false", "diff"];
    if cached {
        arguments.push("--cached");
    }
    arguments.extend([
        "--relative",
        "--no-color",
        "-U3",
        "--no-ext-diff",
        "--no-textconv",
        "--src-prefix=a/",
        "--dst-prefix=b/",
        "--",
    ]);
    let output = git(program, root, &arguments)?;
    if !output.status.success() {
        return Err(DiffError::GitFailed {
            command: "diff",
            message: one_line(&String::from_utf8_lossy(&output.stderr)),
        });
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(split_chunks(&text)
        .into_iter()
        .map(|chunk| file_change(chunk, width, theme))
        .collect())
}

/// Split one `git diff` run into one chunk per file.
fn split_chunks(text: &str) -> Vec<&str> {
    let mut starts = Vec::new();
    if text.starts_with("diff --git ") {
        starts.push(0);
    }
    starts.extend(
        text.match_indices("\ndiff --git ")
            .map(|(index, _)| index + 1),
    );
    let mut chunks = Vec::with_capacity(starts.len());
    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(text.len());
        chunks.push(&text[*start..end]);
    }
    chunks
}

/// One file's chunk: its parsed diff, or the note that replaces it.
fn file_change(chunk: &str, width: usize, theme: &DiffTheme) -> FileChange {
    match parse_unified(chunk) {
        Some(diff) => {
            let (additions, deletions) = counts(&diff);
            let path = if diff.path.is_empty() {
                chunk_path(chunk)
            } else {
                diff.path.clone()
            };
            FileChange {
                path,
                rendered: sanitize(render_diff(&diff, width, theme)),
                note: None,
                additions,
                deletions,
            }
        }
        None => FileChange {
            path: chunk_path(chunk),
            rendered: Vec::new(),
            note: Some(note_for(chunk)),
            additions: 0,
            deletions: 0,
        },
    }
}

fn counts(diff: &FileDiff) -> (usize, usize) {
    let mut additions = 0;
    let mut deletions = 0;
    for line in diff.hunks.iter().flat_map(|hunk| &hunk.lines) {
        match line.kind {
            DiffKind::Add => additions += 1,
            DiffKind::Remove => deletions += 1,
            DiffKind::Context => {}
        }
    }
    (additions, deletions)
}

fn note_for(chunk: &str) -> String {
    if chunk.contains("GIT binary patch") || chunk.contains("Binary files ") {
        "binary file skipped".to_owned()
    } else if chunk.contains("Submodule ") {
        "submodule change; not shown".to_owned()
    } else if chunk.contains("old mode ") || chunk.contains("new mode ") {
        "file mode changed; no content diff".to_owned()
    } else if chunk.contains("rename from ") || chunk.contains("similarity index 100%") {
        "renamed without content changes".to_owned()
    } else {
        "no textual changes".to_owned()
    }
}

fn chunk_path(chunk: &str) -> String {
    for prefix in ["+++ ", "--- "] {
        for line in chunk.lines() {
            let Some(rest) = line.strip_prefix(prefix) else {
                continue;
            };
            let path = rest.split('\t').next().unwrap_or(rest).trim();
            if path != "/dev/null" {
                return strip_side_prefix(path).to_owned();
            }
        }
    }
    let Some(first) = chunk.lines().next() else {
        return "unknown file".to_owned();
    };
    let Some(rest) = first.strip_prefix("diff --git ") else {
        return "unknown file".to_owned();
    };
    if let Some((_, new)) = rest.rsplit_once(" b/") {
        return new.to_owned();
    }
    rest.rsplit_once(' ')
        .map_or_else(|| rest.to_owned(), |(_, new)| new.to_owned())
}

fn strip_side_prefix(path: &str) -> &str {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
}

fn untracked_files(
    program: &str,
    root: &Path,
    width: usize,
    theme: &DiffTheme,
) -> Result<Vec<FileChange>, DiffError> {
    let output = git(
        program,
        root,
        &[
            "-c",
            "core.quotepath=false",
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
        ],
    )?;
    if !output.status.success() {
        return Err(DiffError::GitFailed {
            command: "ls-files",
            message: one_line(&String::from_utf8_lossy(&output.stderr)),
        });
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    Ok(listing
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(|path| untracked_file(root, path, width, theme))
        .collect())
}

fn untracked_file(root: &Path, path: &str, width: usize, theme: &DiffTheme) -> FileChange {
    let full = root.join(path);
    let metadata = match std::fs::symlink_metadata(&full) {
        Ok(metadata) => metadata,
        Err(error) => {
            return note(
                path,
                format!("could not read: {}", one_line(&error.to_string())),
            );
        }
    };
    if metadata.file_type().is_symlink() {
        return note(path, "symbolic link; not shown");
    }
    if !metadata.is_file() {
        return note(path, "not a regular file; not shown");
    }
    if metadata.len() > MAX_UNTRACKED_BYTES {
        return note(
            path,
            format!("{} bytes is too large to show", metadata.len()),
        );
    }
    let bytes = match std::fs::read(&full) {
        Ok(bytes) => bytes,
        Err(error) => {
            return note(
                path,
                format!("could not read: {}", one_line(&error.to_string())),
            );
        }
    };
    if bytes.contains(&0) {
        return note(path, "binary file skipped");
    }
    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return note(path, "empty file; nothing to show");
    }

    let mut text = format!(
        "--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n",
        lines.len()
    );
    for line in &lines {
        text.push('+');
        text.push_str(line);
        text.push('\n');
    }
    let Some(diff) = parse_unified(&text) else {
        return note(path, "could not render the file");
    };
    FileChange {
        path: path.to_owned(),
        rendered: sanitize(render_diff(&diff, width, theme)),
        note: None,
        additions: lines.len(),
        deletions: 0,
    }
}

fn note(path: &str, note: impl Into<String>) -> FileChange {
    FileChange {
        path: path.to_owned(),
        rendered: Vec::new(),
        note: Some(note.into()),
        additions: 0,
        deletions: 0,
    }
}

/// Keep the section within its file and line caps.
fn capped(section: DiffSection, files: Vec<FileChange>) -> SectionView {
    let file_count = files.len();
    let additions = files.iter().map(|file| file.additions).sum();
    let deletions = files.iter().map(|file| file.deletions).sum();
    let total_lines: usize = files.iter().map(|file| file.rendered.len()).sum();

    let mut kept = Vec::new();
    let mut shown_lines = 0;
    for mut file in files {
        if kept.len() >= MAX_FILES || shown_lines >= MAX_LINES {
            break;
        }
        let room = MAX_LINES - shown_lines;
        if file.rendered.len() > room {
            file.rendered.truncate(room);
            shown_lines = MAX_LINES;
            kept.push(file);
            break;
        }
        shown_lines += file.rendered.len();
        kept.push(file);
    }

    SectionView {
        section,
        file_count,
        additions,
        deletions,
        withheld_files: file_count.saturating_sub(kept.len()),
        withheld_lines: total_lines.saturating_sub(shown_lines),
        files: kept,
    }
}

/// Reset every tab to four spaces and drop control characters, so a file
/// cannot misalign the gutter or drive the terminal.
fn sanitize(lines: Vec<RenderedLine>) -> Vec<RenderedLine> {
    lines
        .into_iter()
        .map(|mut line| {
            for span in &mut line.spans {
                let mut text = String::with_capacity(span.text.len());
                for character in span.text.chars() {
                    match character {
                        '\t' => text.push_str("    "),
                        character if character.is_control() => {}
                        character => text.push(character),
                    }
                }
                span.text = text;
            }
            line
        })
        .collect()
}

fn one_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        "no error output".to_owned()
    } else {
        line.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_HUNKS: &str = "\
diff --git a/src/main.rs b/src/main.rs
index 1111111..2222222 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@
 fn main() {
-    old();
+    new();
 }
@@ -10,2 +10,3 @@ context heading
 context
+added
 tail
";

    fn temp_repo() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("tempdir");
        run_git(temp.path(), &["init", "-q"]);
        temp
    }

    fn run_git(root: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["-c", "user.email=test@example.com"])
            .args(["-c", "user.name=Test"])
            .args(arguments)
            .status()
            .expect("git runs");
        assert!(status.success(), "git {arguments:?} failed");
    }

    #[test]
    fn a_two_hunk_file_renders_both_headers_and_its_path() {
        let file = file_change(TWO_HUNKS, RENDER_WIDTH, &DiffTheme::default());
        assert_eq!(file.path, "src/main.rs");
        assert_eq!(file.additions, 2);
        assert_eq!(file.deletions, 1);
        assert_eq!(file.rendered.len(), 9, "two hunk headers and seven lines");
        let headers = file
            .rendered
            .iter()
            .filter(|line| line.kind == niki_render::LineKind::DiffHunk)
            .count();
        assert_eq!(headers, 2);
    }

    #[test]
    fn an_untracked_file_becomes_an_all_added_diff() {
        let temp = temp_repo();
        std::fs::write(temp.path().join("notes.txt"), "first\nsecond\n").expect("write");

        let views = collect(
            temp.path(),
            &[DiffSection::Untracked],
            RENDER_WIDTH,
            &DiffTheme::default(),
        )
        .expect("collect");

        assert_eq!(views.len(), 1);
        let view = &views[0];
        assert_eq!(view.file_count, 1);
        assert_eq!(view.additions, 2);
        assert_eq!(view.deletions, 0);
        let file = &view.files[0];
        assert_eq!(file.path, "notes.txt");
        assert_eq!(file.note, None);
        let texts: Vec<String> = file.rendered.iter().map(RenderedLine::text).collect();
        assert_eq!(texts[0], "@@ -0,0 +1,2 @@");
        assert_eq!(texts[1], "  1 │+first");
        assert_eq!(texts[2], "  2 │+second");
    }

    #[test]
    fn staged_and_unstaged_are_separate_sections() {
        let temp = temp_repo();
        let root = temp.path();
        std::fs::write(root.join("tracked.txt"), "base\n").expect("write");
        run_git(root, &["add", "tracked.txt"]);
        run_git(root, &["commit", "-qm", "base"]);

        std::fs::write(root.join("staged.txt"), "staged\n").expect("write");
        run_git(root, &["add", "staged.txt"]);
        std::fs::write(root.join("tracked.txt"), "changed\n").expect("write");

        let views = collect(
            root,
            &[DiffSection::Staged, DiffSection::Unstaged],
            RENDER_WIDTH,
            &DiffTheme::default(),
        )
        .expect("collect");

        let staged: Vec<&str> = views[0]
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        let unstaged: Vec<&str> = views[1]
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        assert_eq!(staged, ["staged.txt"]);
        assert_eq!(unstaged, ["tracked.txt"]);
        assert_eq!(views[0].additions, 1);
        assert_eq!(views[1].additions, 1);
        assert_eq!(views[1].deletions, 1);
    }

    #[test]
    fn a_binary_file_is_one_note_rather_than_its_content() {
        let temp = temp_repo();
        let root = temp.path();
        std::fs::write(root.join("image.bin"), [0u8, 1, 2, 3]).expect("write");

        let views = collect(
            root,
            &[DiffSection::Untracked],
            RENDER_WIDTH,
            &DiffTheme::default(),
        )
        .expect("collect");
        let file = &views[0].files[0];
        assert_eq!(file.path, "image.bin");
        assert!(file.rendered.is_empty());
        assert_eq!(file.note.as_deref(), Some("binary file skipped"));
    }

    #[test]
    fn a_tracked_binary_change_is_also_one_note() {
        let temp = temp_repo();
        let root = temp.path();
        std::fs::write(root.join("image.bin"), [0u8, 1, 2, 3]).expect("write");
        run_git(root, &["add", "image.bin"]);
        run_git(root, &["commit", "-qm", "image"]);
        std::fs::write(root.join("image.bin"), [0u8, 9, 9, 9]).expect("write");

        let views = collect(
            root,
            &[DiffSection::Unstaged],
            RENDER_WIDTH,
            &DiffTheme::default(),
        )
        .expect("collect");
        let file = &views[0].files[0];
        assert_eq!(file.path, "image.bin");
        assert!(file.rendered.is_empty(), "the bytes are never dumped");
        assert_eq!(file.note.as_deref(), Some("binary file skipped"));
    }

    #[test]
    fn a_missing_git_is_one_line_and_not_a_panic() {
        let temp = temp_repo();
        let error = collect_with(
            "niki-does-not-have-this-git",
            temp.path(),
            &[DiffSection::Staged],
            RENDER_WIDTH,
            &DiffTheme::default(),
        )
        .expect_err("no git");
        assert!(
            error.to_string().contains("git is not installed"),
            "{error}"
        );
    }

    #[test]
    fn a_directory_outside_a_repository_is_one_line() {
        let temp = tempfile::tempdir().expect("tempdir");
        let error = collect(
            temp.path(),
            &[DiffSection::Staged],
            RENDER_WIDTH,
            &DiffTheme::default(),
        )
        .expect_err("not a repository");
        let message = error.to_string();
        assert!(message.starts_with("not a git repository: "), "{message}");
        assert_eq!(message.lines().count(), 1);
    }

    #[test]
    fn the_cap_reports_what_it_withheld() {
        let temp = temp_repo();
        let content: String = (0..500).map(|line| format!("line {line}\n")).collect();
        std::fs::write(temp.path().join("big.txt"), content).expect("write");

        let views = collect(
            temp.path(),
            &[DiffSection::Untracked],
            RENDER_WIDTH,
            &DiffTheme::default(),
        )
        .expect("collect");
        let view = &views[0];
        assert_eq!(view.file_count, 1);
        assert_eq!(view.files[0].rendered.len(), MAX_LINES);
        assert_eq!(view.withheld_lines, 501 - MAX_LINES);
        assert_eq!(view.withheld_files, 0);
        assert_eq!(
            withheld_line(view.withheld_files, view.withheld_lines).as_deref(),
            Some("101 more lines withheld")
        );
    }

    #[test]
    fn the_file_cap_counts_files_it_did_not_show() {
        let temp = temp_repo();
        for index in 0..(MAX_FILES + 1) {
            std::fs::write(temp.path().join(format!("f{index}.txt")), "x\n").expect("write");
        }

        let views = collect(
            temp.path(),
            &[DiffSection::Untracked],
            RENDER_WIDTH,
            &DiffTheme::default(),
        )
        .expect("collect");
        let view = &views[0];
        assert_eq!(view.file_count, MAX_FILES + 1);
        assert_eq!(view.files.len(), MAX_FILES);
        assert_eq!(view.withheld_files, 1);
        assert_eq!(view.withheld_lines, 2, "the withheld file's two lines");
        assert_eq!(
            withheld_line(view.withheld_files, view.withheld_lines).as_deref(),
            Some("1 more file and 2 more lines withheld")
        );
        assert_eq!(view.additions, MAX_FILES + 1);
    }

    #[test]
    fn a_clean_tree_has_no_files_and_plain_text_says_so() {
        let temp = temp_repo();
        let views = collect(
            temp.path(),
            &DiffSection::ALL,
            RENDER_WIDTH,
            &DiffTheme::default(),
        )
        .expect("collect");
        assert!(views.iter().all(|view| view.files.is_empty()));
        assert_eq!(plain_text(&views), "no changes");
    }

    #[test]
    fn the_section_name_is_its_argument() {
        assert_eq!(
            DiffSection::from_argument("STAGED"),
            Some(DiffSection::Staged)
        );
        assert_eq!(
            DiffSection::from_argument(" unstaged "),
            Some(DiffSection::Unstaged)
        );
        assert_eq!(DiffSection::from_argument("all"), None);
    }
}
