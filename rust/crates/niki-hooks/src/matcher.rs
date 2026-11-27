//! Matcher groups from `hooks.json`.

use regex::Regex;

/// One compiled matcher. `None` on a group means "match every target".
#[derive(Debug, Clone)]
pub(crate) enum Matcher {
    /// Exact names separated by `|` or `,`, compared without pattern syntax.
    Names(Vec<String>),
    /// Any other matcher is a regular expression searched inside the target.
    Pattern(Regex),
}

impl Matcher {
    /// Compile the `matcher` field: omitted, empty or `*` matches everything;
    /// a string made only of word characters, whitespace, `-`, `,` and `|` is a
    /// list of exact names; anything else is an unanchored regular expression.
    pub(crate) fn compile(raw: &str) -> Result<Option<Self>, String> {
        if raw.is_empty() || raw == "*" {
            return Ok(None);
        }
        if is_exact_matcher(raw) {
            let names = raw
                .split(['|', ','])
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(str::to_owned)
                .collect();
            return Ok(Some(Self::Names(names)));
        }
        Regex::new(raw)
            .map(|pattern| Some(Self::Pattern(pattern)))
            .map_err(|error| format!("invalid hook matcher {raw:?}: {error}"))
    }

    pub(crate) fn matches(&self, target: &str) -> bool {
        match self {
            Self::Names(names) => names.iter().any(|name| name == target),
            Self::Pattern(pattern) => pattern.is_match(target),
        }
    }
}

/// The exact-match character set: `^[\w\s,\-|]+$`.
fn is_exact_matcher(raw: &str) -> bool {
    raw.chars().all(|ch| {
        ch.is_alphanumeric() || ch == '_' || ch.is_whitespace() || matches!(ch, ',' | '-' | '|')
    })
}

#[cfg(test)]
mod tests {
    use super::Matcher;

    #[test]
    fn an_omitted_or_star_matcher_matches_everything() {
        assert!(Matcher::compile("").expect("compile").is_none());
        assert!(Matcher::compile("*").expect("compile").is_none());
    }

    #[test]
    fn alternation_is_exact_and_regexes_are_searched() {
        let names = Matcher::compile("Bash|Write")
            .expect("compile")
            .expect("matcher");
        assert!(names.matches("Bash"));
        assert!(names.matches("Write"));
        assert!(!names.matches("WriteAgain"));

        let pattern = Matcher::compile("^mcp__.*__write$")
            .expect("compile")
            .expect("matcher");
        assert!(pattern.matches("mcp__fixture__write"));
        assert!(!pattern.matches("mcp__fixture__read"));
    }

    #[test]
    fn an_invalid_regex_is_reported_not_swallowed() {
        assert!(Matcher::compile("(unclosed").is_err());
    }
}
