//! `niki sessions`: the saved sessions, for a user who never opens the
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
            "{}  {}  {}  {} {noun}",
            summary.id,
            summary.updated_at,
            display_title(summary.title.as_deref()),
            summary.messages,
        ));
        if summary.archived {
            out.push_str("  [archived]");
        }
        out.push('\n');
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
                "archived": summary.archived,
            })
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_owned())
}

/// The JSON form of `sessions fork`: the new id and the session it came from.
///
/// The keys are written in the documented order; `serde_json::Value` escapes
/// each id, which is all a caller needs to splice the line into a script.
#[must_use]
pub fn fork_json(id: &str, from: &str) -> String {
    let id = serde_json::Value::from(id).to_string();
    let from = serde_json::Value::from(from).to_string();
    format!("{{\"id\": {id}, \"from\": {from}}}")
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
    fn a_fork_reports_both_ids() {
        assert_eq!(
            fork_json("new-id", "old-id"),
            r#"{"id": "new-id", "from": "old-id"}"#
        );

        let value: serde_json::Value =
            serde_json::from_str(&fork_json("new-id", "old-id")).expect("JSON object");
        assert_eq!(value["id"], "new-id");
        assert_eq!(value["from"], "old-id");
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
