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
}

/// Every non-alias command, in a stable order for help.
pub const COMMANDS: &[CommandInfo] = &[
    CommandInfo {
        name: "/clear",
        summary: "Start a fresh session and empty the view",
    },
    CommandInfo {
        name: "/compact",
        summary: "Fold older rows out of the view; the session store keeps them",
    },
    CommandInfo {
        name: "/help",
        summary: "This list",
    },
    CommandInfo {
        name: "/model",
        summary: "Choose a model, or name one with /model provider:model",
    },
    CommandInfo {
        name: "/quit",
        summary: "Leave the app",
    },
    CommandInfo {
        name: "/resume",
        summary: "List saved sessions, or resume one by id or index",
    },
    CommandInfo {
        name: "/theme",
        summary: "Choose a palette, or cycle to the next one",
    },
    CommandInfo {
        name: "/tools",
        summary: "List the registered tools",
    },
    CommandInfo {
        name: "/version",
        summary: "Show the version",
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
        "/quit" | "/exit" => SlashCommand::Quit,
        "/help" | "/keys" => SlashCommand::Help,
        "/theme" => SlashCommand::Theme(argument),
        "/model" => SlashCommand::Model(argument),
        "/clear" => SlashCommand::Clear,
        "/compact" => SlashCommand::Compact,
        "/resume" => SlashCommand::Resume(argument),
        "/tools" => SlashCommand::Tools,
        "/version" => SlashCommand::Version,
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
            (SlashCommand::Version, "/version"),
        ];
        for (expected, name) in commands {
            assert_eq!(parse(name), Ok(expected), "{name}");
        }
    }

    #[test]
    fn the_command_list_and_the_parser_agree() {
        let listed: Vec<&str> = COMMANDS.iter().map(|command| command.name).collect();
        assert_eq!(listed.len(), 9);
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
        assert_eq!(parse("/keys"), Ok(SlashCommand::Help));
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
