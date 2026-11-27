//! Skills for the Niki Agent: discovery, front matter, and prompt injection.
//!
//! A skill is a directory holding a `SKILL.md`: a YAML front-matter block with a
//! `name` and a `description`, then instructions for a recurring job. The model
//! sees the name, the description and where to read the file; it reads the
//! instructions only when the skill applies, so a long body costs nothing on a
//! turn that does not use it.
//!
//! Discovery takes the first match for a name, in the order the Python client
//! layers its sources:
//!
//! 1. `<workspace>/.niki/skills/<name>/SKILL.md`
//! 2. `<user skills>/<name>/SKILL.md` — the host passes `~/.niki/skills`
//! 3. the built-in skills compiled into this crate with [`include_str!`]
//!
//! A project skill therefore hides a user skill of the same name, and both hide
//! the built-in. A skill with no description, or front matter that does not
//! parse, is a [`SkillWarning`] and the session continues: one broken file must
//! not cost the person their other skills (RULES.md 4).
//!
//! Built-in skills ship inside the binary, so unlike a file skill they have no
//! path the model could read. Their instructions are short and are included in
//! the injected section; [`SkillCatalog::inject_into`] is the only place a body
//! is inlined, and a test holds every built-in body to a small size.

#![forbid(unsafe_code)]

mod error;
mod frontmatter;
mod skill;

pub use error::SkillError;
pub use skill::{Origin, Skill};

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::skill::Source;

/// The file every skill directory holds.
const SKILL_FILE: &str = "SKILL.md";

/// How much of one description reaches the system prompt.
///
/// The skill format allows 1024 characters. A longer description is cut here
/// rather than sent whole: the section is paid for on every request, and the
/// full text stays visible in `/skills`.
const MAX_DESCRIPTION_CHARS: usize = 1024;

/// A skill compiled into the binary.
struct BuiltIn {
    /// The directory name, used when the front matter has no name.
    directory: &'static str,
    /// The whole `SKILL.md`, front matter included.
    text: &'static str,
}

/// Built-in skills, lowest precedence, in list order.
const BUILT_INS: [BuiltIn; 2] = [
    BuiltIn {
        directory: "commit",
        text: include_str!("../builtin/commit/SKILL.md"),
    },
    BuiltIn {
        directory: "review",
        text: include_str!("../builtin/review/SKILL.md"),
    },
];

/// A skill that was skipped, or loaded with part of it dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillWarning {
    /// The `SKILL.md` that caused it, or the skills directory that could not be
    /// listed.
    pub path: PathBuf,
    /// What was wrong, written for the person running the command.
    pub reason: String,
}

impl fmt::Display for SkillWarning {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path.display(), self.reason)
    }
}

/// Every skill one session can see, in precedence order.
#[derive(Debug, Clone, Default)]
pub struct SkillCatalog {
    skills: Vec<Skill>,
    warnings: Vec<SkillWarning>,
}

impl SkillCatalog {
    /// Discover the project, user and built-in skills.
    ///
    /// `workspace_root` is the directory the tools are rooted at; the project
    /// skills are found under its `.niki/skills`. `user_skills` is the user's
    /// skills directory — normally `~/.niki/skills` — so a host that honours
    /// `NIKI_HOME` can point it at the profile in use.
    ///
    /// A missing skills directory is normal and silent. Anything else that keeps
    /// one skill from loading is collected by [`Self::warnings`]; discovery
    /// itself cannot fail, because one broken file must not hide the rest.
    #[must_use]
    pub fn discover(workspace_root: &Path, user_skills: &Path) -> Self {
        let mut catalog = Self::default();
        // A project skill comes from the repository, so it must not follow a
        // symlink out of the workspace: a hostile checkout could otherwise make
        // discovery read any file the person can read, or a device that never
        // ends. The user's own skills directory may be a link into a shared
        // profile, so it is deliberately not bounded.
        let boundary = workspace_root.canonicalize().ok();
        catalog.load_dir(
            &workspace_root.join(".niki").join("skills"),
            Origin::Project,
            boundary.as_deref(),
        );
        catalog.load_dir(user_skills, Origin::User, None);
        catalog.load_built_ins();
        catalog
    }

    /// The skills this session can use, highest precedence first.
    #[must_use]
    pub fn list(&self) -> &[Skill] {
        &self.skills
    }

    /// Whether discovery found nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// The skills that were skipped, and why.
    #[must_use]
    pub fn warnings(&self) -> &[SkillWarning] {
        &self.warnings
    }

    /// Append the compact skills section to a system prompt.
    ///
    /// The section names every skill with its description and, for a skill on
    /// disk, the path to read. Bodies of file skills are not included; a body
    /// can be arbitrarily long and most turns do not use it.
    #[must_use]
    pub fn inject_into(&self, system_prompt: &str) -> String {
        if self.skills.is_empty() {
            return system_prompt.to_owned();
        }
        let base = system_prompt.trim_end();
        let section = self.section();
        if base.is_empty() {
            section
        } else {
            format!("{base}\n\n{section}")
        }
    }

    /// The skills section on its own.
    fn section(&self) -> String {
        let mut out = String::from(
            "## Skills\n\n\
             Instruction packs for recurring jobs. Check this list before starting \
             work; when a skill applies, read its `SKILL.md` at the path shown and \
             follow it. Do not read a skill that does not apply.\n\n",
        );
        for skill in &self.skills {
            let built_in = skill.origin() == Origin::BuiltIn;
            out.push_str(&format!(
                "- **{}**{}: {}",
                skill.name(),
                if built_in { " (built in)" } else { "" },
                one_line(skill.description(), MAX_DESCRIPTION_CHARS),
            ));
            if let Some(path) = skill.path() {
                out.push_str(&format!(
                    "\n  Read `{}` for the full instructions.",
                    path.display()
                ));
            }
            out.push('\n');
        }

        let built_ins: Vec<&Skill> = self
            .skills
            .iter()
            .filter(|skill| skill.origin() == Origin::BuiltIn)
            .collect();
        if !built_ins.is_empty() {
            out.push_str(
                "\nBuilt-in skills ship inside the Niki Agent binary, so they have no \
                 file to read; their instructions are included here.\n",
            );
            for skill in built_ins {
                let instructions = skill.built_in_instructions().unwrap_or_default();
                out.push_str(&format!(
                    "\n### {}\n\n{}\n",
                    skill.name(),
                    instructions.trim()
                ));
            }
        }
        out
    }

    /// Load every skill directory under `directory`.
    ///
    /// When `boundary` is set, the directory and every skill file must resolve
    /// inside it; a symlink that leaves it is skipped with a warning.
    fn load_dir(&mut self, directory: &Path, origin: Origin, boundary: Option<&Path>) {
        if let Some(boundary) = boundary {
            match directory.canonicalize() {
                Ok(resolved) if resolved.starts_with(boundary) => {}
                Ok(_) => {
                    self.warn(
                        directory,
                        "the skills directory resolves outside the workspace",
                    );
                    return;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => return,
                Err(error) => {
                    self.warn(directory, format!("cannot resolve: {error}"));
                    return;
                }
            }
        }

        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return,
            Err(error) => {
                self.warn(directory, format!("cannot list skills: {error}"));
                return;
            }
        };

        let mut directories: Vec<PathBuf> = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    // Follow a symlink only when the target is inside the
                    // boundary: a shared skills directory is linked into the
                    // profile rather than copied there.
                    if path.is_dir() {
                        if let Some(boundary) = boundary {
                            match path.canonicalize() {
                                Ok(resolved) if resolved.starts_with(boundary) => {}
                                Ok(_) => {
                                    self.warn(
                                        &path,
                                        "the skill directory resolves outside the workspace",
                                    );
                                    continue;
                                }
                                Err(error) => {
                                    self.warn(&path, format!("cannot resolve: {error}"));
                                    continue;
                                }
                            }
                        }
                        directories.push(path);
                    }
                }
                Err(error) => {
                    self.warn(directory, format!("cannot read a directory entry: {error}"))
                }
            }
        }
        directories.sort();
        for directory in directories {
            self.load_skill(&directory, origin, boundary);
        }
    }

    /// Load one skill directory, skipping it with a warning when it is broken.
    fn load_skill(&mut self, directory: &Path, origin: Origin, boundary: Option<&Path>) {
        let file = directory.join(SKILL_FILE);
        if let Some(boundary) = boundary {
            match file.canonicalize() {
                Ok(resolved) if resolved.starts_with(boundary) => {}
                Ok(_) => {
                    self.warn(&file, "the skill file resolves outside the workspace");
                    return;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => return,
                Err(error) => {
                    self.warn(&file, format!("cannot resolve: {error}"));
                    return;
                }
            }
        }
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return,
            Err(error) => {
                self.warn(&file, format!("cannot read: {error}"));
                return;
            }
        };
        let name = directory
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        match Skill::parse(&text, name, origin, Source::File(file.clone())) {
            Ok(skill) => self.add(skill),
            Err(error) => self.warn(&file, error.to_string()),
        }
    }

    fn load_built_ins(&mut self) {
        for built_in in &BUILT_INS {
            match Skill::parse(
                built_in.text,
                built_in.directory,
                Origin::BuiltIn,
                Source::Embedded(built_in.text),
            ) {
                Ok(skill) => self.add(skill),
                // A shipped skill that does not parse is a defect in this crate,
                // not in the person's files; the warning names it instead of
                // taking the session down, and a test parses every built-in.
                Err(error) => self.warn(
                    Path::new(built_in.directory),
                    format!("built-in skill does not parse: {error}"),
                ),
            }
        }
    }

    /// Keep the first skill with a name; a later source of the same name loses.
    fn add(&mut self, skill: Skill) {
        if self
            .skills
            .iter()
            .any(|existing| existing.name == skill.name)
        {
            return;
        }
        self.skills.push(skill);
    }

    fn warn(&mut self, path: &Path, reason: impl Into<String>) {
        self.warnings.push(SkillWarning {
            path: path.to_path_buf(),
            reason: reason.into(),
        });
    }
}

/// Collapse whitespace and cut a description to `limit` characters.
fn one_line(text: &str, limit: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= limit {
        return collapsed;
    }
    let cut: String = collapsed.chars().take(limit).collect();
    format!("{cut}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &Path, name: &str, text: &str) {
        let directory = root.join(name);
        std::fs::create_dir_all(&directory).expect("skill directory");
        std::fs::write(directory.join(SKILL_FILE), text).expect("skill file");
    }

    fn described(name: &str, description: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\n\nbody for {name}\n")
    }

    fn discovered(project: &Path, user: &Path) -> SkillCatalog {
        SkillCatalog::discover(project, user)
    }

    #[test]
    fn a_project_skill_hides_the_user_and_built_in_skills_of_the_same_name() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        write_skill(
            &project.path().join(".niki/skills"),
            "commit",
            &described("commit", "project commit"),
        );
        write_skill(user.path(), "commit", &described("commit", "user commit"));

        let catalog = discovered(project.path(), user.path());
        let commits: Vec<&Skill> = catalog
            .list()
            .iter()
            .filter(|skill| skill.name() == "commit")
            .collect();
        assert_eq!(commits.len(), 1, "a name is listed once");
        assert_eq!(commits[0].description(), "project commit");
        assert_eq!(commits[0].origin(), Origin::Project);
        assert_eq!(
            commits[0].path(),
            Some(
                project
                    .path()
                    .join(".niki/skills/commit/SKILL.md")
                    .as_path()
            )
        );
    }

    #[test]
    fn a_user_skill_hides_the_built_in_of_the_same_name() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        write_skill(user.path(), "review", &described("review", "user review"));

        let catalog = discovered(project.path(), user.path());
        let review: Vec<&Skill> = catalog
            .list()
            .iter()
            .filter(|skill| skill.name() == "review")
            .collect();
        assert_eq!(review.len(), 1);
        assert_eq!(review[0].description(), "user review");
        assert_eq!(review[0].origin(), Origin::User);
    }

    #[test]
    fn the_built_ins_fill_in_when_nothing_overrides_them() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let catalog = discovered(project.path(), user.path());

        let names: Vec<&str> = catalog.list().iter().map(Skill::name).collect();
        assert_eq!(names, ["commit", "review"]);
        assert!(
            catalog
                .list()
                .iter()
                .all(|skill| skill.origin() == Origin::BuiltIn)
        );
        assert!(catalog.warnings().is_empty(), "{:?}", catalog.warnings());
        assert!(catalog.list()[0].path().is_none());
    }

    #[test]
    fn discovery_is_silent_when_no_skills_directory_exists() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let catalog = discovered(project.path(), user.path());
        assert!(catalog.warnings().is_empty());
        assert_eq!(catalog.list().len(), BUILT_INS.len());
    }

    #[test]
    fn a_skill_without_front_matter_is_skipped_with_a_warning() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let skills = project.path().join(".niki/skills");
        write_skill(&skills, "broken", "# just markdown\n");
        write_skill(&skills, "fine", &described("fine", "a working skill"));

        let catalog = discovered(project.path(), user.path());
        assert!(
            catalog.list().iter().all(|skill| skill.name() != "broken"),
            "the broken skill is not listed"
        );
        assert!(catalog.list().iter().any(|skill| skill.name() == "fine"));
        assert_eq!(catalog.warnings().len(), 1, "{:?}", catalog.warnings());
        let warning = &catalog.warnings()[0];
        assert!(warning.path.ends_with("broken/SKILL.md"), "{warning}");
        assert!(warning.reason.contains("front matter"), "{warning}");
    }

    #[test]
    fn a_skill_without_a_description_is_skipped_with_a_warning() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        write_skill(
            &project.path().join(".niki/skills"),
            "empty",
            "---\nname: empty\n---\nbody\n",
        );

        let catalog = discovered(project.path(), user.path());
        assert!(catalog.list().iter().all(|skill| skill.name() != "empty"));
        assert_eq!(catalog.warnings().len(), 1);
        assert!(catalog.warnings()[0].reason.contains("description"));
    }

    #[test]
    fn a_directory_without_a_skill_file_is_not_a_skill_and_not_a_warning() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let skills = project.path().join(".niki/skills");
        std::fs::create_dir_all(skills.join("notes")).expect("directory");
        std::fs::write(skills.join("README.md"), "not a skill").expect("file");

        let catalog = discovered(project.path(), user.path());
        assert_eq!(catalog.list().len(), BUILT_INS.len());
        assert!(catalog.warnings().is_empty(), "{:?}", catalog.warnings());
    }

    #[test]
    fn a_missing_name_uses_the_directory_name() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        write_skill(
            &project.path().join(".niki/skills"),
            "foldered",
            "---\ndescription: named by its folder\n---\nbody\n",
        );

        let catalog = discovered(project.path(), user.path());
        let skill = catalog
            .list()
            .iter()
            .find(|skill| skill.name() == "foldered")
            .expect("the folder name is used");
        assert_eq!(skill.description(), "named by its folder");
    }

    #[test]
    fn a_skill_body_is_readable_from_disk() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let text = described("readable", "has a body");
        write_skill(&project.path().join(".niki/skills"), "readable", &text);

        let catalog = discovered(project.path(), user.path());
        let skill = catalog
            .list()
            .iter()
            .find(|skill| skill.name() == "readable")
            .expect("skill");
        assert_eq!(skill.read().expect("read the body"), text);
    }

    #[test]
    fn a_read_that_fails_is_an_error_not_a_panic() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        write_skill(
            &project.path().join(".niki/skills"),
            "gone",
            &described("gone", "read after removal"),
        );

        let catalog = discovered(project.path(), user.path());
        let skill = catalog
            .list()
            .iter()
            .find(|skill| skill.name() == "gone")
            .expect("skill");
        std::fs::remove_file(project.path().join(".niki/skills/gone/SKILL.md")).expect("remove");
        let error = skill.read().expect_err("the file is gone");
        assert!(error.to_string().contains("SKILL.md"), "{error}");
    }

    #[test]
    fn the_injection_keeps_the_prompt_and_adds_the_section() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        write_skill(
            &project.path().join(".niki/skills"),
            "deploy",
            &described("deploy", "Ship a release safely."),
        );

        let catalog = discovered(project.path(), user.path());
        let prompt = catalog.inject_into("You are Niki Agent.\n");
        assert!(
            prompt.starts_with("You are Niki Agent.\n\n## Skills\n"),
            "{prompt}"
        );
        assert!(
            prompt.contains("- **deploy**: Ship a release safely."),
            "{prompt}"
        );
        assert!(prompt.contains("Read `"), "{prompt}");
        assert!(prompt.contains("deploy/SKILL.md"), "{prompt}");
        assert!(
            prompt.contains("read its `SKILL.md` at the path shown"),
            "{prompt}"
        );
    }

    #[test]
    fn the_injection_lists_each_of_twenty_skills_once_and_stays_compact() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let skills = project.path().join(".niki/skills");
        let mut bodies = 0;
        for index in 0..20 {
            let name = format!("job-{index:02}");
            let body = "x".repeat(2_000);
            bodies += body.len();
            write_skill(
                &skills,
                &name,
                &format!(
                    "---\nname: {name}\ndescription: Handle recurring job number {index}.\n---\n{body}\n"
                ),
            );
        }

        let catalog = discovered(project.path(), user.path());
        let prompt = catalog.inject_into("");
        for index in 0..20 {
            let name = format!("job-{index:02}");
            assert_eq!(
                prompt.matches(&format!("- **{name}**")).count(),
                1,
                "{name} is listed once"
            );
        }
        assert!(
            prompt.len() < 8 * 1024,
            "20 skills produced {} bytes",
            prompt.len()
        );
        assert!(
            prompt.len() < bodies / 4,
            "the bodies are not inlined ({} against {bodies})",
            prompt.len()
        );
    }

    #[test]
    fn a_long_body_is_never_inlined_for_a_skill_on_disk() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        write_skill(
            &project.path().join(".niki/skills"),
            "long",
            &format!(
                "---\nname: long\ndescription: Has a long body.\n---\nMARKER-IN-THE-BODY {}\n",
                "y".repeat(20_000)
            ),
        );

        let catalog = discovered(project.path(), user.path());
        let prompt = catalog.inject_into("");
        assert!(
            !prompt.contains("MARKER-IN-THE-BODY"),
            "the body is not inlined"
        );
        assert!(prompt.len() < 4 * 1024, "{} bytes", prompt.len());
    }

    #[test]
    fn a_description_too_long_for_the_prompt_is_cut() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let description = format!("{} TAIL", "d".repeat(5_000));
        write_skill(
            &project.path().join(".niki/skills"),
            "verbose",
            &described("verbose", &description),
        );

        let catalog = discovered(project.path(), user.path());
        let prompt = catalog.inject_into("");
        assert!(!prompt.contains("TAIL"), "the tail is cut");
        assert!(prompt.contains(&"d".repeat(MAX_DESCRIPTION_CHARS)));
        assert_eq!(
            catalog
                .list()
                .iter()
                .find(|skill| skill.name() == "verbose")
                .expect("skill")
                .description(),
            description.trim(),
            "the full description stays available to /skills"
        );
    }

    #[test]
    fn the_built_in_skills_are_usable_and_their_instructions_stay_small() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let catalog = discovered(project.path(), user.path());
        let prompt = catalog.inject_into("");

        assert!(prompt.contains("### commit"), "{prompt}");
        assert!(prompt.contains("### review"), "{prompt}");
        assert!(
            prompt.contains("Conventional Commits"),
            "the commit instructions are present"
        );
        let section = prompt
            .split("their instructions are included here")
            .nth(1)
            .expect("built-in section");
        assert!(
            section.len() < 4 * 1024,
            "built-in instructions are {} bytes",
            section.len()
        );

        for built_in in catalog.list() {
            let text = built_in.read().expect("built-in text");
            assert!(
                text.len() < 2_500,
                "built-in {} is {} bytes",
                built_in.name(),
                text.len()
            );
            assert!(!text.contains("---\nname: \n"), "the built-ins have a name");
        }
    }

    #[test]
    fn an_empty_catalog_leaves_the_system_prompt_alone() {
        let catalog = SkillCatalog::default();
        assert!(catalog.is_empty());
        assert_eq!(catalog.inject_into("base prompt"), "base prompt");
        assert_eq!(catalog.inject_into(""), "");
    }

    #[test]
    fn discovery_order_is_project_then_user_then_built_in() {
        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let project_skills = project.path().join(".niki/skills");
        write_skill(
            &project_skills,
            "zulu",
            &described("zulu", "a project skill"),
        );
        write_skill(
            &project_skills,
            "alpha",
            &described("alpha", "a project skill"),
        );
        write_skill(
            user.path(),
            "user-one",
            &described("user-one", "a user skill"),
        );

        let catalog = discovered(project.path(), user.path());
        let names: Vec<&str> = catalog.list().iter().map(Skill::name).collect();
        assert_eq!(
            names,
            ["alpha", "zulu", "user-one", "commit", "review"],
            "sources are grouped, each alphabetical"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_project_skill_symlinked_out_of_the_workspace_is_not_read() {
        use std::os::unix::fs::symlink;

        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(
            outside.path().join("SKILL.md"),
            described("leaked", "a secret from outside the workspace"),
        )
        .expect("outside skill");

        let skills = project.path().join(".niki/skills");
        std::fs::create_dir_all(&skills).expect("skills");
        symlink(outside.path(), skills.join("directory-link")).expect("directory symlink");
        let file_dir = skills.join("file-link");
        std::fs::create_dir_all(&file_dir).expect("file link directory");
        symlink(outside.path().join("SKILL.md"), file_dir.join(SKILL_FILE)).expect("file symlink");

        let catalog = discovered(project.path(), user.path());
        assert!(
            catalog.list().iter().all(|skill| skill.name() != "leaked"),
            "{:?}",
            catalog.list().iter().map(Skill::name).collect::<Vec<_>>()
        );
        assert!(
            catalog
                .warnings()
                .iter()
                .any(|warning| warning.path.ends_with("directory-link")),
            "{:?}",
            catalog.warnings()
        );
        assert!(
            catalog
                .warnings()
                .iter()
                .any(|warning| warning.path.ends_with("file-link/SKILL.md")),
            "{:?}",
            catalog.warnings()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_project_skill_symlinked_inside_the_workspace_is_still_read() {
        use std::os::unix::fs::symlink;

        let project = tempfile::tempdir().expect("project");
        let user = tempfile::tempdir().expect("user");
        let target = project.path().join("shared/team");
        write_skill(
            project.path(),
            "shared/team",
            &described("deploy", "a shared project skill"),
        );

        let skills = project.path().join(".niki/skills");
        std::fs::create_dir_all(&skills).expect("skills");
        symlink(&target, skills.join("deploy")).expect("symlink");

        let catalog = discovered(project.path(), user.path());
        assert!(
            catalog.list().iter().any(|skill| skill.name() == "deploy"),
            "{:?}",
            catalog.warnings()
        );
    }
}
