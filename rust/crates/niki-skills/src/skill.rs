use std::path::{Path, PathBuf};

use crate::error::{ParseError, SkillError};
use crate::frontmatter;

/// Which of the three places a skill was found in.
///
/// The order is the precedence order: a project skill hides a user skill of the
/// same name, which hides a built-in one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// `.niki/skills` under the workspace root.
    Project,
    /// The user's `~/.niki/skills`.
    User,
    /// Shipped inside the binary.
    BuiltIn,
}

impl Origin {
    /// The word shown to the person running the command.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::User => "user",
            Self::BuiltIn => "built-in",
        }
    }
}

/// Where a skill's instructions live.
#[derive(Debug, Clone)]
pub(crate) enum Source {
    /// A `SKILL.md` on disk.
    File(PathBuf),
    /// A `SKILL.md` compiled into the binary, with its front matter, so the
    /// body can be shown without a file to read.
    Embedded(&'static str),
}

/// One skill: its metadata, its origin, and where to read the full file.
#[derive(Debug, Clone)]
pub struct Skill {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) origin: Origin,
    pub(crate) source: Source,
}

impl Skill {
    /// The name from the front matter, or the directory name when it has none.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The one-line description from the front matter.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Where the skill was found.
    #[must_use]
    pub fn origin(&self) -> Origin {
        self.origin
    }

    /// The `SKILL.md` path for a skill on disk, or `None` for a built-in.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        match &self.source {
            Source::File(path) => Some(path),
            Source::Embedded(_) => None,
        }
    }

    /// Where to read the full instructions: a path, or `built-in`.
    #[must_use]
    pub fn location(&self) -> String {
        match self.path() {
            Some(path) => path.display().to_string(),
            None => Origin::BuiltIn.label().to_owned(),
        }
    }

    /// Read the full `SKILL.md`: from disk for a file skill, from the binary
    /// for a built-in.
    pub fn read(&self) -> Result<String, SkillError> {
        match &self.source {
            Source::File(path) => {
                std::fs::read_to_string(path).map_err(|source| SkillError::Read {
                    path: path.clone(),
                    source,
                })
            }
            Source::Embedded(text) => Ok((*text).to_owned()),
        }
    }

    /// The instructions after the front matter, for a built-in skill.
    ///
    /// Built-ins ship inside the binary, so there is no file for the model to
    /// read on demand; their instructions are short enough to sit in the system
    /// prompt, and this is how they get there.
    pub(crate) fn built_in_instructions(&self) -> Option<&'static str> {
        match &self.source {
            Source::Embedded(text) => Some(frontmatter::parse(text).ok()?.instructions),
            Source::File(_) => None,
        }
    }

    /// Parse one `SKILL.md`.
    ///
    /// `directory` supplies the name when the front matter has none, which is
    /// what makes a skill folder usable before anyone fills in the field.
    pub(crate) fn parse(
        text: &str,
        directory: &str,
        origin: Origin,
        source: Source,
    ) -> Result<Self, ParseError> {
        let front = frontmatter::parse(text)?;
        let name = front
            .name
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .or_else(|| non_empty(directory))
            .ok_or(ParseError::MissingName)?;
        let description = front
            .description
            .map(|description| description.trim().to_owned())
            .filter(|description| !description.is_empty())
            .ok_or(ParseError::MissingDescription)?;
        Ok(Self {
            name,
            description,
            origin,
            source,
        })
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str, directory: &str) -> Result<Skill, ParseError> {
        Skill::parse(
            text,
            directory,
            Origin::Project,
            Source::File(PathBuf::from("/work/.niki/skills/x/SKILL.md")),
        )
    }

    #[test]
    fn the_front_matter_name_wins_over_the_directory_name() {
        let skill = parse("---\nname: commit\ndescription: d\n---\n", "folder").expect("skill");
        assert_eq!(skill.name(), "commit");
        assert_eq!(skill.origin(), Origin::Project);
        assert_eq!(
            skill.path().map(Path::to_path_buf),
            Some(PathBuf::from("/work/.niki/skills/x/SKILL.md"))
        );
    }

    #[test]
    fn a_missing_name_falls_back_to_the_directory_name() {
        let skill = parse("---\ndescription: d\n---\n", "folder").expect("skill");
        assert_eq!(skill.name(), "folder");
        let no_name = parse("---\nname: \"  \"\ndescription: d\n---\n", "folder").expect("skill");
        assert_eq!(no_name.name(), "folder");
    }

    #[test]
    fn a_missing_or_empty_description_is_rejected() {
        assert_eq!(
            parse("---\nname: x\n---\n", "x").unwrap_err(),
            ParseError::MissingDescription
        );
        assert_eq!(
            parse("---\nname: x\ndescription: \"  \"\n---\n", "x").unwrap_err(),
            ParseError::MissingDescription
        );
    }

    #[test]
    fn a_skill_with_no_name_at_all_is_rejected() {
        assert_eq!(
            parse("---\ndescription: d\n---\n", "").unwrap_err(),
            ParseError::MissingName
        );
    }

    #[test]
    fn an_embedded_skill_reads_from_the_binary_and_has_no_path() {
        let skill = Skill::parse(
            "---\nname: commit\ndescription: d\n---\nbody",
            "commit",
            Origin::BuiltIn,
            Source::Embedded("---\nname: commit\ndescription: d\n---\nbody"),
        )
        .expect("skill");
        assert_eq!(skill.path(), None);
        assert_eq!(skill.location(), "built-in");
        assert_eq!(
            skill.read().expect("read"),
            "---\nname: commit\ndescription: d\n---\nbody"
        );
        assert_eq!(skill.built_in_instructions(), Some("body"));
    }
}
