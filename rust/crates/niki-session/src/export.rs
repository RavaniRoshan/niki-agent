//! Markdown export of a saved session.
//!
//! A transcript is a person's own conversation, so the export writes it
//! verbatim: the prompt and the answer keep their text, and a tool result is
//! the only thing shortened, because a single result can dwarf every turn
//! around it.
//!
//! The file is always named inside the workspace, and a name that already
//! exists is never replaced: the second export of a session lands beside the
//! first as `-2`.

use std::borrow::Cow;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use crate::{Message, SessionError, SessionId, SessionStore, Summary};

/// Bytes of one tool result the Markdown keeps.
pub const RESULT_LIMIT: usize = 2 * 1024;

/// The marker appended to a result the limit cut.
pub const TRUNCATED: &str = "[truncated]";

/// Why a session could not be exported.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error("refusing to write {path}: it is outside the workspace {root}")]
    OutsideRoot { root: String, path: String },
    #[error("cannot write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Write `session` as Markdown and return the path written.
///
/// Without `requested` the name is `niki-<session-id>.md` in `root`. A
/// `requested` path resolves inside `root`, and one that leaves it is refused.
/// A name already on disk gains `-2`, `-3`, and so on.
///
/// # Errors
///
/// Returns [`ExportError::OutsideRoot`] for a path that leaves the workspace,
/// [`ExportError::Session`] when the transcript cannot be read, and
/// [`ExportError::Io`] when the file cannot be written.
pub fn write(
    store: &SessionStore,
    session: &SessionId,
    root: &Path,
    requested: Option<&str>,
) -> Result<PathBuf, ExportError> {
    let summary = store
        .find(session.as_str())?
        .ok_or_else(|| SessionError::UnknownSession {
            id: session.clone(),
        })?;
    let messages = store.load(session)?;
    let text = markdown(&summary, &messages);

    let root = root.canonicalize().map_err(|source| ExportError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    let candidate = match requested {
        Some(raw) => resolve(&root, raw)?,
        None => root.join(format!("niki-{}.md", session.as_str())),
    };
    write_unique(&candidate, &text)
}

/// The Markdown a session reads as.
///
/// The heading is the title, or the first prompt, or a placeholder when the
/// session has neither. Every turn is `## You` and `## Niki`, and a tool result
/// is a fenced block whose info string names the tool that produced it.
#[must_use]
pub fn markdown(summary: &Summary, messages: &[Message]) -> String {
    let mut out = String::new();
    out.push_str("# ");
    out.push_str(heading(summary));
    out.push_str("\n\n");

    for message in messages {
        match message.role.as_str() {
            "user" => {
                out.push_str("## You\n\n");
                out.push_str(&message.content);
                out.push_str("\n\n");
            }
            "assistant" => {
                out.push_str("## Niki\n\n");
                out.push_str(&message.content);
                out.push_str("\n\n");
            }
            "tool_result" | "tool_error" => {
                let (name, result) = split_record(&message.content);
                out.push_str("```");
                out.push_str(fence_label(name));
                out.push('\n');
                out.push_str(&clip(result));
                out.push_str("\n```\n\n");
            }
            _ => {}
        }
    }

    let mut text = out.trim_end().to_owned();
    text.push('\n');
    text
}

/// The name the export gives the session: the title first, then the first
/// prompt, and only then a placeholder.
fn heading(summary: &Summary) -> &str {
    named(summary.title.as_deref())
        .or_else(|| named(summary.first_prompt.as_deref()))
        .unwrap_or("Untitled session")
}

/// The first non-empty line of a session's name.
fn named(text: Option<&str>) -> Option<&str> {
    text.and_then(|text| text.lines().map(str::trim).find(|line| !line.is_empty()))
}

/// A stored tool row is its name on the first line and the rest below it.
fn split_record(content: &str) -> (&str, &str) {
    content.split_once('\n').unwrap_or((content, ""))
}

/// A fence's info string, which Markdown reads as one word.
fn fence_label(name: &str) -> &str {
    let name = name.trim();
    if name.is_empty() || name.chars().any(|character| character.is_whitespace()) {
        "text"
    } else {
        name
    }
}

/// Keep at most [`RESULT_LIMIT`] bytes of a result and mark where it stopped.
fn clip(text: &str) -> Cow<'_, str> {
    if text.len() <= RESULT_LIMIT {
        return Cow::Borrowed(text);
    }
    let mut end = RESULT_LIMIT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut clipped = String::with_capacity(end + TRUNCATED.len() + 2);
    clipped.push_str(&text[..end]);
    clipped.push('\n');
    clipped.push_str(TRUNCATED);
    Cow::Owned(clipped)
}

/// Resolve a requested path and refuse one that leaves the workspace.
///
/// The path is normalised lexically; `..` may not climb above the root, and an
/// absolute path is only accepted when it is already inside it. The parent
/// directory must exist, and its canonical form must stay inside the root, so
/// a symlinked directory cannot carry the write elsewhere.
fn resolve(root: &Path, raw: &str) -> Result<PathBuf, ExportError> {
    let joined = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        root.join(raw)
    };
    let mut normalised = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => normalised.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(part) => normalised.push(part),
            Component::ParentDir => {
                normalised.pop();
            }
        }
    }

    let outside = || ExportError::OutsideRoot {
        root: root.display().to_string(),
        path: raw.to_owned(),
    };
    if !normalised.starts_with(root) {
        return Err(outside());
    }
    let name = normalised.file_name().ok_or_else(outside)?.to_os_string();
    let parent = normalised.parent().ok_or_else(outside)?;
    let parent = parent.canonicalize().map_err(|source| ExportError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    if !parent.starts_with(root) {
        return Err(outside());
    }
    Ok(parent.join(name))
}

/// Create `path`, or the first free `-2`, `-3`, … beside it.
///
/// `create_new` makes the no-overwrite rule hold even when another process
/// names the same file between a check and a write.
fn write_unique(path: &Path, text: &str) -> Result<PathBuf, ExportError> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().map_or_else(
        || "niki-export".to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned());

    let mut attempt = 1_u32;
    loop {
        let candidate = if attempt == 1 {
            path.to_path_buf()
        } else {
            let name = match &extension {
                Some(extension) => format!("{stem}-{attempt}.{extension}"),
                None => format!("{stem}-{attempt}"),
            };
            parent.join(name)
        };
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                file.write_all(text.as_bytes())
                    .map_err(|source| ExportError::Io {
                        path: candidate.clone(),
                        source,
                    })?;
                return Ok(candidate);
            }
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                attempt = attempt.wrapping_add(1);
            }
            Err(source) => {
                return Err(ExportError::Io {
                    path: candidate,
                    source,
                });
            }
        }
    }
}
