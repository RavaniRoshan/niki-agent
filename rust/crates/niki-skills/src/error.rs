use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Why a skill's instructions could not be read.
///
/// Discovery never fails as a whole: a `SKILL.md` that cannot be read is a
/// [`crate::SkillWarning`], so one broken skill does not hide the rest
/// (RULES.md 4).
#[derive(Debug, Error)]
pub enum SkillError {
    /// The `SKILL.md` could not be read from disk.
    #[error("cannot read {path}: {source}")]
    Read {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying I/O failure.
        #[source]
        source: io::Error,
    },
}

/// Why one `SKILL.md` was not turned into a skill.
///
/// Kept separate from [`SkillError`] because these are content problems found
/// during discovery, and discovery reports them as warnings rather than
/// returning them to the caller.
#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum ParseError {
    /// The file does not start with a `---` line.
    #[error("no front matter: the file must start with a `---` line")]
    NoFrontMatter,

    /// The opening `---` line has no closing `---` line.
    #[error("the front matter is not closed with a `---` line")]
    UnterminatedFrontMatter,

    /// A front-matter line outside a nested block is not `key: value`.
    #[error("{line:?} in the front matter is not a `key: value` pair")]
    NotAKeyValue {
        /// The offending line, so the warning points at what to fix.
        line: String,
    },

    /// Neither the front matter nor the directory supplied a name.
    #[error("the skill has no name and its directory has none either")]
    MissingName,

    /// The front matter has no `description`, or it is empty.
    #[error("the front matter has no description")]
    MissingDescription,
}
