//! The front matter of a `SKILL.md`.
//!
//! A skill file starts with a YAML block between `---` lines; this product reads
//! two fields from it, `name` and `description`. Rather than depend on a full
//! YAML parser for two keys, the reader here accepts the shape the skill format
//! uses — top-level `key: value`, quoted or plain, with `|` and `>` block
//! scalars — and reports anything else as a warning instead of guessing at it.
//! A skill that asks for a nested structure still loads; its nested lines are
//! skipped because they are not one of the two fields.

use crate::error::ParseError;

/// The line that opens and closes a front-matter block.
const FENCE: &str = "---";

/// The fields this product reads, and the instructions after the block.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct FrontMatter<'a> {
    /// The `name` field, when it was present and non-empty.
    pub(crate) name: Option<String>,
    /// The `description` field, when it was present.
    pub(crate) description: Option<String>,
    /// Everything after the closing `---` line.
    pub(crate) instructions: &'a str,
}

/// Split a `SKILL.md` into its fields and its instructions.
///
/// The first line must be `---`, and the block ends at the next `---` line. Line
/// endings may be LF or CRLF.
pub(crate) fn parse(text: &str) -> Result<FrontMatter<'_>, ParseError> {
    let (first, mut rest) = take_line(text);
    if first.trim_end() != FENCE {
        return Err(ParseError::NoFrontMatter);
    }

    let mut fields = FrontMatter::default();
    loop {
        if rest.is_empty() {
            return Err(ParseError::UnterminatedFrontMatter);
        }
        let (line, after) = take_line(rest);
        rest = after;

        if line.trim_end() == FENCE {
            fields.instructions = rest;
            return Ok(fields);
        }
        // An empty line or a comment carries no field.
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        // An indented line belongs to a nested value (`metadata:`, a list, or a
        // scalar continuation). Those are not fields this reader owns, and
        // guessing at them would turn a valid skill into a different one.
        if line.starts_with([' ', '\t']) {
            continue;
        }

        let Some((key, value)) = line.split_once(':') else {
            return Err(ParseError::NotAKeyValue {
                line: line.to_owned(),
            });
        };
        let key = key.trim();
        let value = value.trim();
        let value = if is_block_scalar(value) {
            let (text, after) = block_scalar(value, rest);
            rest = after;
            text
        } else {
            unquote(value)
        };
        match key {
            "name" => fields.name = non_empty(value),
            "description" => fields.description = non_empty(value),
            _ => {}
        }
    }
}

/// One line without its line ending, and the text after that line.
fn take_line(text: &str) -> (&str, &str) {
    match text.split_once('\n') {
        Some((line, rest)) => (line.strip_suffix('\r').unwrap_or(line), rest),
        None => (text, ""),
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.trim().to_owned())
}

/// Strip one matching pair of surrounding quotes.
///
/// Only a way to read `description: "a: b"` as one value; YAML escapes inside
/// the quotes are left alone because the text is shown, not evaluated.
fn unquote(value: &str) -> String {
    let bytes = value.as_bytes();
    let quoted = value.len() >= 2
        && ((bytes[0] == b'"' && bytes[value.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[value.len() - 1] == b'\''));
    if quoted {
        value[1..value.len() - 1].to_owned()
    } else {
        value.trim().to_owned()
    }
}

fn is_block_scalar(value: &str) -> bool {
    matches!(value, "|" | "|-" | "|+" | ">" | ">-" | ">+")
}

/// Read the indented lines of a `|` (literal) or `>` (folded) scalar.
///
/// The first non-empty line sets the indentation every line is measured
/// against, which is what YAML does and what makes a block scalar written with
/// four spaces work the same as one written with two.
fn block_scalar<'a>(indicator: &str, text: &'a str) -> (String, &'a str) {
    let mut collected: Vec<&str> = Vec::new();
    let mut rest = text;
    loop {
        if rest.is_empty() {
            break;
        }
        let (line, after) = take_line(rest);
        let closes = line.trim_end() == FENCE;
        if closes || (!line.is_empty() && !line.starts_with([' ', '\t'])) {
            break;
        }
        collected.push(line);
        rest = after;
    }

    let indent = collected
        .iter()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .unwrap_or(0);
    let stripped: Vec<&str> = collected
        .iter()
        .map(|line| line.get(indent..).unwrap_or_else(|| line.trim_start()))
        .collect();
    let joined = if indicator.starts_with('>') {
        stripped.join(" ")
    } else {
        stripped.join("\n")
    };
    (joined.trim().to_owned(), rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_block_yields_both_fields_and_the_instructions() {
        let text = "---\nname: commit\ndescription: Write a commit.\n---\n\n# Body\n";
        let front = parse(text).expect("front matter");
        assert_eq!(front.name.as_deref(), Some("commit"));
        assert_eq!(front.description.as_deref(), Some("Write a commit."));
        assert_eq!(front.instructions, "\n# Body\n");
    }

    #[test]
    fn a_missing_or_unclosed_block_is_an_error() {
        assert_eq!(
            parse("# no front matter\n").unwrap_err(),
            ParseError::NoFrontMatter
        );
        assert_eq!(parse("").unwrap_err(), ParseError::NoFrontMatter);
        assert_eq!(
            parse("---\nname: x\n").unwrap_err(),
            ParseError::UnterminatedFrontMatter
        );
    }

    #[test]
    fn a_line_that_is_not_a_pair_is_rejected_with_the_line() {
        let error = parse("---\nname: x\nnonsense\n---\n").unwrap_err();
        assert_eq!(
            error,
            ParseError::NotAKeyValue {
                line: "nonsense".to_owned()
            }
        );
        assert!(error.to_string().contains("nonsense"));
    }

    #[test]
    fn quotes_are_stripped_and_a_colon_inside_a_value_is_kept() {
        let front = parse("---\ndescription: \"Commit: explain why\"\nname: 'x'\n---\n")
            .expect("front matter");
        assert_eq!(front.description.as_deref(), Some("Commit: explain why"));
        assert_eq!(front.name.as_deref(), Some("x"));
        let plain = parse("---\ndescription: a: b\n---\n").expect("front matter");
        assert_eq!(plain.description.as_deref(), Some("a: b"));
    }

    #[test]
    fn block_scalars_are_folded_like_yaml() {
        let literal =
            parse("---\ndescription: |\n  first\n  second\n---\nbody").expect("front matter");
        assert_eq!(literal.description.as_deref(), Some("first\nsecond"));
        assert_eq!(literal.instructions, "body");

        let folded = parse("---\ndescription: >-\n  first\n  second\n---\n").expect("front matter");
        assert_eq!(folded.description.as_deref(), Some("first second"));

        let indented = parse("---\nname: x\ndescription: |\n    four\n    spaces\n---\n")
            .expect("front matter");
        assert_eq!(indented.description.as_deref(), Some("four\nspaces"));
    }

    #[test]
    fn nested_values_are_skipped_and_do_not_become_fields() {
        let text =
            "---\nname: x\ndescription: d\nmetadata:\n  include_tools: a b\ntags:\n  - one\n---\n";
        let front = parse(text).expect("front matter");
        assert_eq!(front.name.as_deref(), Some("x"));
        assert_eq!(front.description.as_deref(), Some("d"));
    }

    #[test]
    fn crlf_line_endings_are_read_like_lf() {
        let text = "---\r\nname: x\r\ndescription: d\r\n---\r\nbody\r\n";
        let front = parse(text).expect("front matter");
        assert_eq!(front.name.as_deref(), Some("x"));
        assert_eq!(front.description.as_deref(), Some("d"));
        assert_eq!(front.instructions, "body\r\n");
    }

    #[test]
    fn the_last_duplicate_field_wins() {
        let front =
            parse("---\nname: first\ndescription: a\nname: second\n---\n").expect("front matter");
        assert_eq!(front.name.as_deref(), Some("second"));
    }

    #[test]
    fn a_fence_may_have_trailing_spaces() {
        let front = parse("---   \nname: x\ndescription: d\n---   \n").expect("front matter");
        assert_eq!(front.name.as_deref(), Some("x"));
    }
}
