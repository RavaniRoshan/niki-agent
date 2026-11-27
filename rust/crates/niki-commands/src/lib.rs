//! The slash commands the composer speaks.
//!
//! The grammar is pure so the screen stays terminal-free: the app parses a line
//! that starts with `/` and acts on the variant. Everything a person can type is
//! listed in [`COMMANDS`], which is also what the help overlay draws, so the
//! list and the parser cannot drift apart.

#![forbid(unsafe_code)]

use std::fmt;

use niki_policy::Mode;

/// One command the shell understands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    /// Switch to this approval mode for the running session.
    Mode(Mode),
    /// Leave the application.
    Quit,
    /// Show the key and command list.
    Help,
    /// Select a palette by name, or cycle when none is named.
    Theme(Option<String>),
    /// Open the model picker, or switch to a spec directly.
    Model(Option<String>),
    /// Empty the transcript view and start a new session.
    Clear,
    /// Fold older transcript rows out of the view, keeping the session store.
    Compact,
    /// Show the conversation's size and what the session has spent.
    Context,
    /// Show the working tree's staged, unstaged and untracked changes.
    Diff(Option<String>),
    /// List saved sessions, or resume one by id or one-based index.
    Resume(Option<String>),
    /// Browse saved sessions and preview their first prompt.
    Threads,
    /// Write the current session as Markdown, or to the path named.
    Export(Option<String>),
    /// Name the current session, or show its title.
    Rename(Option<String>),
    /// List the registered tools.
    Tools,
    /// List the skills discovery found.
    Skills,
    /// Show the running session's model, mode, paths and counts.
    Status,
    /// Write an `AGENTS.md` template when the workspace has none.
    Init,
    /// Show the version line.
    Version,
}

/// A command name and what it does, in words for the help overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandInfo {
    /// The name as it is typed, leading slash included.
    pub name: &'static str,
    /// One line for the help overlay.
    pub summary: &'static str,
    /// Other spellings that reach the same handler.
    pub aliases: &'static [&'static str],
}

/// Every command the parser accepts, in a stable order for help.
pub const COMMANDS: &[CommandInfo] = &[
    CommandInfo {
        name: "/auto",
        summary: "Switch to Auto approval mode",
        aliases: &[],
    },
    CommandInfo {
        name: "/clear",
        summary: "Start a fresh session and empty the view",
        aliases: &[],
    },
    CommandInfo {
        name: "/compact",
        summary: "Summarise older turns to keep the conversation within the context window",
        aliases: &[],
    },
    CommandInfo {
        name: "/context",
        summary: "Show the conversation's size and the session's token total",
        aliases: &[],
    },
    CommandInfo {
        name: "/diff",
        summary: "Show the working tree's staged, unstaged and untracked changes",
        aliases: &[],
    },
    CommandInfo {
        name: "/export",
        summary: "Write the session to Markdown",
        aliases: &[],
    },
    CommandInfo {
        name: "/help",
        summary: "This list",
        aliases: &["/keys"],
    },
    CommandInfo {
        name: "/init",
        summary: "Write an AGENTS.md template when the workspace has none",
        aliases: &[],
    },
    CommandInfo {
        name: "/manual",
        summary: "Switch to Manual approval mode",
        aliases: &[],
    },
    CommandInfo {
        name: "/model",
        summary: "Choose a model, or name one with /model provider:model",
        aliases: &[],
    },
    CommandInfo {
        name: "/quit",
        summary: "Leave the app",
        aliases: &["/exit", "/q"],
    },
    CommandInfo {
        name: "/rename",
        summary: "Name the current session, or show its title",
        aliases: &[],
    },
    CommandInfo {
        name: "/resume",
        summary: "List saved sessions, or resume one by id or index",
        aliases: &[],
    },
    CommandInfo {
        name: "/skills",
        summary: "List project and user skills",
        aliases: &[],
    },
    CommandInfo {
        name: "/status",
        summary: "Show the running session's model, mode, paths and counts",
        aliases: &[],
    },
    CommandInfo {
        name: "/theme",
        summary: "Choose a palette, or cycle to the next one",
        aliases: &[],
    },
    CommandInfo {
        name: "/threads",
        summary: "Browse saved sessions and preview their first prompt",
        aliases: &[],
    },
    CommandInfo {
        name: "/tools",
        summary: "List the registered tools",
        aliases: &[],
    },
    CommandInfo {
        name: "/version",
        summary: "Show the version",
        aliases: &["/about"],
    },
    CommandInfo {
        name: "/yolo",
        summary: "Switch to YOLO approval mode",
        aliases: &[],
    },
];

/// Commands the earlier client had that this build does not answer yet.
///
/// `parse` rejects these. The list exists so the help screen and
/// `rust/PARITY.md` can say what is missing without promising it, and so a
/// command moves out of here the day it is implemented.
pub const ROADMAP: &[CommandInfo] = &[
    CommandInfo {
        name: "/agents",
        summary: "Browse and switch between agents",
        aliases: &[],
    },
    CommandInfo {
        name: "/auth",
        summary: "Store and check provider credentials",
        aliases: &["/connect"],
    },
    CommandInfo {
        name: "/btw",
        summary: "Ask a side question without changing the conversation",
        aliases: &[],
    },
    CommandInfo {
        name: "/context-doctor",
        summary: "Report what a session injects and its estimated token cost",
        aliases: &[],
    },
    CommandInfo {
        name: "/copy",
        summary: "Copy the latest answer",
        aliases: &[],
    },
    CommandInfo {
        name: "/cost",
        summary: "Show the token cost of this session",
        aliases: &[],
    },
    CommandInfo {
        name: "/docs",
        summary: "Open the documentation",
        aliases: &[],
    },
    CommandInfo {
        name: "/editor",
        summary: "Compose the prompt in $EDITOR",
        aliases: &[],
    },
    CommandInfo {
        name: "/effort",
        summary: "Set reasoning effort for the current model",
        aliases: &[],
    },
    CommandInfo {
        name: "/feedback",
        summary: "Open the issue tracker",
        aliases: &[],
    },
    CommandInfo {
        name: "/goal",
        summary: "Set a persistent objective with a finish line",
        aliases: &[],
    },
    CommandInfo {
        name: "/line-numbers",
        summary: "Show or hide line numbers in diffs",
        aliases: &[],
    },
    CommandInfo {
        name: "/mcp",
        summary: "List MCP servers and their state",
        aliases: &[],
    },
    CommandInfo {
        name: "/plugins",
        summary: "Manage plugins",
        aliases: &[],
    },
    CommandInfo {
        name: "/prompts",
        summary: "Search and reuse an earlier prompt",
        aliases: &[],
    },
    CommandInfo {
        name: "/remember",
        summary: "Save useful context as a skill or note",
        aliases: &[],
    },
    CommandInfo {
        name: "/scrollbar",
        summary: "Show or hide the transcript scrollbar",
        aliases: &[],
    },
    CommandInfo {
        name: "/timestamps",
        summary: "Show or hide message timestamps",
        aliases: &[],
    },
    CommandInfo {
        name: "/tokens",
        summary: "Show token usage for this session",
        aliases: &[],
    },
    CommandInfo {
        name: "/update",
        summary: "Check for and install an update",
        aliases: &[],
    },
];

/// A line that is not one of the commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownCommand {
    name: String,
}

impl UnknownCommand {
    /// The word that was not recognised.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for UnknownCommand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = COMMANDS.iter().map(|command| command.name).collect();
        write!(
            formatter,
            "there is no {} command; try {}",
            self.name,
            names.join(", ")
        )
    }
}

impl std::error::Error for UnknownCommand {}

/// Parse one line typed into the composer.
///
/// Only the first word is the command. `/rename` and `/export` take the rest of
/// the line as their argument, because a title and a path can both hold spaces;
/// every other command takes only the second word, and a command that takes no
/// argument ignores anything after it.
///
/// # Errors
///
/// Returns [`UnknownCommand`] naming the first word when it is not a command.
pub fn parse(text: &str) -> Result<SlashCommand, UnknownCommand> {
    let mut words = text.split_whitespace();
    let name = words.next().unwrap_or_default();
    let argument = words.next().map(str::to_owned);
    let command = match name {
        "/manual" => SlashCommand::Mode(Mode::Manual),
        "/auto" => SlashCommand::Mode(Mode::Auto),
        "/yolo" => SlashCommand::Mode(Mode::Yolo),
        "/quit" | "/exit" | "/q" => SlashCommand::Quit,
        "/help" | "/keys" => SlashCommand::Help,
        "/theme" => SlashCommand::Theme(argument),
        "/model" => SlashCommand::Model(argument),
        "/clear" => SlashCommand::Clear,
        "/compact" => SlashCommand::Compact,
        "/context" => SlashCommand::Context,
        "/diff" => SlashCommand::Diff(argument),
        "/resume" => SlashCommand::Resume(argument),
        "/threads" => SlashCommand::Threads,
        "/export" => SlashCommand::Export(remainder(text)),
        "/rename" => SlashCommand::Rename(remainder(text)),
        "/tools" => SlashCommand::Tools,
        "/skills" => SlashCommand::Skills,
        "/status" => SlashCommand::Status,
        "/init" => SlashCommand::Init,
        "/version" | "/about" => SlashCommand::Version,
        other => {
            return Err(UnknownCommand {
                name: other.to_owned(),
            });
        }
    };
    Ok(command)
}

/// Everything after the command word, trimmed, when anything is left.
fn remainder(text: &str) -> Option<String> {
    let mut words = text.trim_start().splitn(2, char::is_whitespace);
    let _name = words.next();
    let rest = words.next().unwrap_or_default().trim();
    (!rest.is_empty()).then(|| rest.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_command_parses_to_its_own_variant() {
        let commands: Vec<(SlashCommand, &str)> = vec![
            (SlashCommand::Mode(Mode::Auto), "/auto"),
            (SlashCommand::Clear, "/clear"),
            (SlashCommand::Compact, "/compact"),
            (SlashCommand::Context, "/context"),
            (SlashCommand::Diff(None), "/diff"),
            (SlashCommand::Export(None), "/export"),
            (SlashCommand::Help, "/help"),
            (SlashCommand::Init, "/init"),
            (SlashCommand::Mode(Mode::Manual), "/manual"),
            (SlashCommand::Model(None), "/model"),
            (SlashCommand::Quit, "/quit"),
            (SlashCommand::Rename(None), "/rename"),
            (SlashCommand::Resume(None), "/resume"),
            (SlashCommand::Theme(None), "/theme"),
            (SlashCommand::Threads, "/threads"),
            (SlashCommand::Tools, "/tools"),
            (SlashCommand::Skills, "/skills"),
            (SlashCommand::Status, "/status"),
            (SlashCommand::Init, "/init"),
            (SlashCommand::Version, "/version"),
            (SlashCommand::Mode(Mode::Yolo), "/yolo"),
        ];
        for (expected, name) in commands {
            assert_eq!(parse(name), Ok(expected), "{name}");
        }
    }

    #[test]
    fn the_command_list_and_the_parser_agree() {
        let listed: Vec<&str> = COMMANDS.iter().map(|command| command.name).collect();
        assert_eq!(listed.len(), 20);
        for (index, info) in COMMANDS.iter().enumerate() {
            assert!(parse(info.name).is_ok(), "{} does not parse", info.name);
            assert!(!info.summary.is_empty(), "{} has no summary", info.name);
            for other in COMMANDS.iter().skip(index + 1) {
                assert_ne!(info.name, other.name, "duplicate command name");
                assert_ne!(info.summary, other.summary, "duplicate summary");
            }
        }
    }

    #[test]
    fn context_parses_and_the_tables_keep_their_invariants() {
        assert_eq!(parse("/context"), Ok(SlashCommand::Context));
        assert!(COMMANDS.iter().any(|info| info.name == "/context"));
        assert!(!ROADMAP.iter().any(|info| info.name == "/context"));

        let names: Vec<&str> = COMMANDS.iter().map(|info| info.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "COMMANDS stays sorted for the help overlay");
    }

    #[test]
    fn status_parses_and_is_listed_once() {
        assert_eq!(parse("/status"), Ok(SlashCommand::Status));
        assert!(COMMANDS.iter().any(|info| info.name == "/status"));
        assert!(!ROADMAP.iter().any(|info| info.name == "/status"));
    }

    #[test]
    fn init_parses_and_is_listed_once() {
        assert_eq!(parse("/init"), Ok(SlashCommand::Init));
        assert!(COMMANDS.iter().any(|info| info.name == "/init"));
        assert!(!ROADMAP.iter().any(|info| info.name == "/init"));
    }

    #[test]
    fn rename_and_export_take_the_rest_of_the_line() {
        assert_eq!(parse("/rename"), Ok(SlashCommand::Rename(None)));
        assert_eq!(parse("/rename   "), Ok(SlashCommand::Rename(None)));
        assert_eq!(
            parse("/rename session two"),
            Ok(SlashCommand::Rename(Some("session two".to_owned())))
        );
        assert_eq!(
            parse("  /rename\tplan the port  "),
            Ok(SlashCommand::Rename(Some("plan the port".to_owned())))
        );
        assert_eq!(parse("/export"), Ok(SlashCommand::Export(None)));
        assert_eq!(
            parse("/export notes/today.md"),
            Ok(SlashCommand::Export(Some("notes/today.md".to_owned())))
        );
    }

    #[test]
    fn rename_and_export_are_implemented_rather_than_planned() {
        for name in ["/rename", "/export"] {
            assert!(COMMANDS.iter().any(|info| info.name == name), "{name}");
            assert!(!ROADMAP.iter().any(|info| info.name == name), "{name}");
        }
    }

    #[test]
    fn the_mode_commands_parse_to_the_mode_they_name() {
        assert_eq!(parse("/manual"), Ok(SlashCommand::Mode(Mode::Manual)));
        assert_eq!(parse("/auto"), Ok(SlashCommand::Mode(Mode::Auto)));
        assert_eq!(parse("/yolo"), Ok(SlashCommand::Mode(Mode::Yolo)));
        assert_eq!(
            parse("/auto now"),
            Ok(SlashCommand::Mode(Mode::Auto)),
            "a mode command takes no argument"
        );
        for (name, mode) in [
            ("/manual", Mode::Manual),
            ("/auto", Mode::Auto),
            ("/yolo", Mode::Yolo),
        ] {
            assert!(COMMANDS.iter().any(|info| info.name == name), "{name}");
            assert!(!ROADMAP.iter().any(|info| info.name == name), "{name}");
            assert_eq!(parse(name), Ok(SlashCommand::Mode(mode)), "{name}");
        }
    }

    #[test]
    fn threads_parses_and_is_no_longer_planned() {
        assert_eq!(parse("/threads"), Ok(SlashCommand::Threads));
        assert!(COMMANDS.iter().any(|info| info.name == "/threads"));
        assert!(!ROADMAP.iter().any(|info| info.name == "/threads"));
    }

    #[test]
    fn diff_parses_with_its_optional_section() {
        assert_eq!(parse("/diff"), Ok(SlashCommand::Diff(None)));
        assert_eq!(
            parse("/diff staged"),
            Ok(SlashCommand::Diff(Some("staged".to_owned())))
        );
        assert!(COMMANDS.iter().any(|info| info.name == "/diff"));
        assert!(!ROADMAP.iter().any(|info| info.name == "/diff"));
    }

    #[test]
    fn the_aliases_keep_working() {
        assert_eq!(parse("/exit"), Ok(SlashCommand::Quit));
        assert_eq!(parse("/q"), Ok(SlashCommand::Quit));
        assert_eq!(parse("/keys"), Ok(SlashCommand::Help));
        assert_eq!(parse("/about"), Ok(SlashCommand::Version));
    }

    #[test]
    fn every_alias_is_listed_with_its_command_and_parses_to_it() {
        for info in COMMANDS {
            let canonical = parse(info.name).expect("listed command parses");
            for alias in info.aliases {
                assert_eq!(
                    parse(alias),
                    Ok(canonical.clone()),
                    "{alias} should reach {}",
                    info.name
                );
            }
        }
    }

    #[test]
    fn the_roadmap_names_nothing_the_parser_answers_and_nothing_twice() {
        for info in ROADMAP {
            assert!(
                parse(info.name).is_err(),
                "{} is on the roadmap but parses today; move it to COMMANDS",
                info.name
            );
            assert!(!info.summary.is_empty(), "{} has no summary", info.name);
        }
        for (index, info) in ROADMAP.iter().enumerate() {
            assert!(
                !COMMANDS.iter().any(|done| done.name == info.name),
                "{} is both implemented and planned",
                info.name
            );
            for other in ROADMAP.iter().skip(index + 1) {
                assert_ne!(info.name, other.name, "duplicate roadmap name");
            }
        }
    }

    #[test]
    fn arguments_are_captured_for_the_commands_that_take_one() {
        assert_eq!(
            parse("/theme mono"),
            Ok(SlashCommand::Theme(Some("mono".to_owned())))
        );
        assert_eq!(
            parse("/model groq:llama"),
            Ok(SlashCommand::Model(Some("groq:llama".to_owned())))
        );
        assert_eq!(
            parse("/resume 2"),
            Ok(SlashCommand::Resume(Some("2".to_owned())))
        );
        assert_eq!(
            parse("/clear ignored"),
            Ok(SlashCommand::Clear),
            "a command without arguments ignores the rest"
        );
    }

    #[test]
    fn surrounding_whitespace_and_newlines_are_ignored() {
        assert_eq!(parse("  /tools  \n"), Ok(SlashCommand::Tools));
        assert_eq!(parse("\t/version\t"), Ok(SlashCommand::Version));
    }

    #[test]
    fn an_unknown_word_names_itself_and_every_command() {
        let error = parse("/nope").expect_err("unknown");
        assert_eq!(error.name(), "/nope");
        let text = error.to_string();
        assert!(text.contains("/nope"), "{text}");
        for info in COMMANDS {
            assert!(
                text.contains(info.name),
                "{} missing from {text}",
                info.name
            );
        }
    }

    #[test]
    fn an_empty_line_is_unknown_rather_than_a_command() {
        assert!(parse("").is_err());
        assert!(parse("hello").is_err());
    }
}
