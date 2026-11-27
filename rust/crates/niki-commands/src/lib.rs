//! The slash commands the composer speaks.
//!
//! The grammar is pure so the screen stays terminal-free: the app parses a line
//! that starts with `/` and acts on the variant. Everything a person can type is
//! listed in [`COMMANDS`], which is also what the help overlay draws, so the
//! list and the parser cannot drift apart.

#![forbid(unsafe_code)]

use std::fmt;

/// One command the shell understands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
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
    /// List saved sessions, or resume one by id or one-based index.
    Resume(Option<String>),
    /// List the registered tools.
    Tools,
    /// List the skills discovery found.
    Skills,
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
        name: "/clear",
        summary: "Start a fresh session and empty the view",
        aliases: &[],
    },
    CommandInfo {
        name: "/compact",
        summary: "Fold older rows out of the view; the session store keeps them",
        aliases: &[],
    },
    CommandInfo {
        name: "/help",
        summary: "This list",
        aliases: &["/keys"],
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
        name: "/theme",
        summary: "Choose a palette, or cycle to the next one",
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
        name: "/auto",
        summary: "Switch to Auto approval mode",
        aliases: &[],
    },
    CommandInfo {
        name: "/btw",
        summary: "Ask a side question without changing the conversation",
        aliases: &[],
    },
    CommandInfo {
        name: "/context",
        summary: "Show how much of the context window is in use",
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
        name: "/manual",
        summary: "Switch to Manual approval mode",
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
        name: "/threads",
        summary: "Browse and resume earlier sessions",
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
    CommandInfo {
        name: "/yolo",
        summary: "Switch to YOLO approval mode",
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
/// Only the first word is the command and only the second is its argument; a
/// command that takes no argument ignores anything after it.
///
/// # Errors
///
/// Returns [`UnknownCommand`] naming the first word when it is not a command.
pub fn parse(text: &str) -> Result<SlashCommand, UnknownCommand> {
    let mut words = text.split_whitespace();
    let name = words.next().unwrap_or_default();
    let argument = words.next().map(str::to_owned);
    let command = match name {
        "/quit" | "/exit" | "/q" => SlashCommand::Quit,
        "/help" | "/keys" => SlashCommand::Help,
        "/theme" => SlashCommand::Theme(argument),
        "/model" => SlashCommand::Model(argument),
        "/clear" => SlashCommand::Clear,
        "/compact" => SlashCommand::Compact,
        "/resume" => SlashCommand::Resume(argument),
        "/tools" => SlashCommand::Tools,
        "/skills" => SlashCommand::Skills,
        "/version" | "/about" => SlashCommand::Version,
        other => {
            return Err(UnknownCommand {
                name: other.to_owned(),
            });
        }
    };
    Ok(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_command_parses_to_its_own_variant() {
        let commands: Vec<(SlashCommand, &str)> = vec![
            (SlashCommand::Clear, "/clear"),
            (SlashCommand::Compact, "/compact"),
            (SlashCommand::Help, "/help"),
            (SlashCommand::Model(None), "/model"),
            (SlashCommand::Quit, "/quit"),
            (SlashCommand::Resume(None), "/resume"),
            (SlashCommand::Theme(None), "/theme"),
            (SlashCommand::Tools, "/tools"),
            (SlashCommand::Skills, "/skills"),
            (SlashCommand::Version, "/version"),
        ];
        for (expected, name) in commands {
            assert_eq!(parse(name), Ok(expected), "{name}");
        }
    }

    #[test]
    fn the_command_list_and_the_parser_agree() {
        let listed: Vec<&str> = COMMANDS.iter().map(|command| command.name).collect();
        assert_eq!(listed.len(), 10);
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
