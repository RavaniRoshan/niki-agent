//! `niki sessions list`: the saved sessions, for a user who never opens the
//! full-screen app.
//!
//! The list is read-only. The caller resolves `sessions.db` and the workspace
//! filter; this module only turns store summaries into the two output forms.
//! Timestamps are the stored Unix seconds, because that is the format the
//! session schema keeps.

use niki_session::Summary;

/// The placeholder a session without a usable title shows.
const UNTITLED: &str = "(untitled)";

/// The plain-text form: one line per session, already newest first.
#[must_use]
pub fn text(summaries: &[Summary]) -> String {
    if summaries.is_empty() {
        return "no saved sessions\n".to_owned();
    }
    let mut out = String::new();
    for summary in summaries {
        let noun = if summary.messages == 1 {
            "message"
        } else {
            "messages"
        };
        out.push_str(&format!(
            "{}  {}  {}  {} {noun}\n",
            summary.id,
            summary.updated_at,
            display_title(summary.title.as_deref()),
            summary.messages,
        ));
    }
    out
}

/// The JSON form: one array with a row per session, or `[]` when there are
/// none.
#[must_use]
pub fn json(summaries: &[Summary]) -> String {
    let rows: Vec<serde_json::Value> = summaries
        .iter()
        .map(|summary| {
            serde_json::json!({
                "id": summary.id.as_str(),
                "updated_at": summary.updated_at,
                "title": summary.title,
                "messages": summary.messages,
                "workspace": summary.cwd,
            })
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_owned())
}

/// A title folded onto one line, with the placeholder when it is absent or
/// nothing but whitespace.
fn display_title(title: Option<&str>) -> String {
    let Some(title) = title else {
        return UNTITLED.to_owned();
    };
    let title: String = title
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    if title.is_empty() {
        UNTITLED.to_owned()
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_list_has_one_empty_line_or_an_empty_array() {
        assert_eq!(text(&[]), "no saved sessions\n");
        assert_eq!(json(&[]), "[]");
    }

    #[test]
    fn a_title_is_made_safe_for_one_line() {
        assert_eq!(display_title(None), "(untitled)");
        assert_eq!(display_title(Some("  ")), "(untitled)");
        assert_eq!(display_title(Some("deploy notes")), "deploy notes");
        assert_eq!(
            display_title(Some("deploy\nnotes\u{1b}[2J")),
            "deploy notes [2J"
        );
    }
}
