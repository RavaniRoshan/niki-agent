//! Small, comment-preserving edits to `config.toml`.
//!
//! The connect screen writes two things: a default model and, for an endpoint
//! the product does not ship, a `[providers.<name>]` block. Both edits are
//! line-based so the rest of the file — comments, ordering, unrelated tables —
//! survives untouched. A full parse-and-reserialise would reorder every table
//! and drop every comment, which is not a trade a person's hand-written file
//! should have to make.

/// Replace `[models].default`, adding the table when it is absent.
#[must_use]
pub fn set_default_model(text: &str, spec: &str) -> String {
    let value = quote(spec);
    let Some(header) = find_table(text, "models") else {
        let mut output = text.to_owned();
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        if !output.is_empty() && !output.ends_with("\n\n") {
            output.push('\n');
        }
        output.push_str("[models]\n");
        output.push_str(&format!("default = {value}\n"));
        return output;
    };

    let end = table_end(text, header);
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let mut out: Vec<String> = lines[..=header].to_vec();
    let mut replaced = false;
    for line in &lines[header + 1..end] {
        if !replaced && key_of(line) == Some("default") {
            out.push(format!("default = {value}"));
            replaced = true;
        } else {
            out.push(line.clone());
        }
    }
    if !replaced {
        out.insert(header + 1, format!("default = {value}"));
    }
    out.extend(lines[end..].iter().cloned());
    finish(out)
}

/// Add or replace one `[providers.<name>]` block at the end of the file.
///
/// The block names the endpoint, the protocol and, when it is not keyless, the
/// environment variable the key would be read from. The key itself belongs in
/// `credentials.toml`, which [`super::credentials`] writes.
#[must_use]
pub fn set_provider(
    text: &str,
    name: &str,
    base_url: &str,
    protocol: &str,
    keyless: bool,
) -> String {
    let header = format!("providers.{name}");
    let mut block = vec![
        format!("[providers.{name}]"),
        format!("base_url = {}", quote(base_url)),
        format!("protocol = {}", quote(protocol)),
    ];
    if keyless {
        block.push("keyless = true".to_owned());
    } else {
        block.push(format!("api_key_env = {}", quote(&env_name(name))));
    }

    let body = match find_table(text, &header) {
        Some(start) => {
            let end = table_end(text, start);
            let lines: Vec<&str> = text.lines().collect();
            let mut out: Vec<String> = lines[..start]
                .iter()
                .map(|line| (*line).to_owned())
                .collect();
            out.extend(block);
            out.extend(lines[end..].iter().map(|line| (*line).to_owned()));
            out
        }
        None => {
            let mut out: Vec<String> = text.lines().map(str::to_owned).collect();
            if out.last().is_some_and(|line| !line.trim().is_empty()) {
                out.push(String::new());
            }
            out.extend(block);
            out
        }
    };
    finish(body)
}

/// The environment variable a custom provider's key would come from.
#[must_use]
pub fn env_name(provider: &str) -> String {
    let mut name = String::new();
    for character in provider.chars() {
        if character.is_ascii_alphanumeric() {
            name.push(character.to_ascii_uppercase());
        } else {
            name.push('_');
        }
    }
    format!("{name}_API_KEY")
}

/// The 0-based line index of a table header, matching `[name]` exactly.
fn find_table(text: &str, name: &str) -> Option<usize> {
    let wanted = format!("[{name}]");
    text.lines().position(|line| line.trim() == wanted.as_str())
}

/// The first line index at or after `header` that starts a new table.
fn table_end(text: &str, header: usize) -> usize {
    let mut index = header + 1;
    for line in text.lines().skip(header + 1) {
        if line.trim_start().starts_with('[') && line.trim_end().ends_with(']') {
            return index;
        }
        index += 1;
    }
    index
}

/// The key on a `key = value` line, when the line has one.
fn key_of(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.starts_with('#') {
        return None;
    }
    let (key, _) = trimmed.split_once('=')?;
    Some(key.trim())
}

/// A TOML basic string.
fn quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Join lines with one trailing newline.
fn finish(lines: Vec<String>) -> String {
    let mut text = lines.join("\n");
    while text.ends_with("\n\n") {
        text.pop();
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_models_table_is_appended() {
        let text = "[providers.groq]\nbase_url = \"https://api.groq.com/openai/v1\"\n";
        let out = set_default_model(text, "groq:llama-3.3-70b");
        assert!(out.contains("[models]\ndefault = \"groq:llama-3.3-70b\""));
        assert!(
            out.contains("[providers.groq]"),
            "the provider block survives"
        );
    }

    #[test]
    fn an_existing_default_is_replaced_in_place() {
        let text = "[models]\n# keep me\ndefault = \"old:model\"\ntemperature = 0.2\n";
        let out = set_default_model(text, "new:model");
        assert_eq!(
            out,
            "[models]\n# keep me\ndefault = \"new:model\"\ntemperature = 0.2\n"
        );
    }

    #[test]
    fn a_models_table_without_a_default_gets_one_after_the_header() {
        let text = "[models]\ntemperature = 0.2\n";
        let out = set_default_model(text, "a:b");
        assert_eq!(out, "[models]\ndefault = \"a:b\"\ntemperature = 0.2\n");
    }

    #[test]
    fn a_key_in_another_table_is_not_mistaken_for_the_default() {
        let text = "[ui]\ndefault = \"dark\"\n\n[models]\n";
        let out = set_default_model(text, "a:b");
        assert!(out.contains("[ui]\ndefault = \"dark\""));
        assert!(out.contains("[models]\ndefault = \"a:b\""));
    }

    #[test]
    fn a_custom_provider_is_appended_and_then_replaced() {
        let text = "[models]\ndefault = \"a:b\"\n";
        let once = set_provider(text, "custom", "https://example.test/v1", "openai", false);
        assert!(once.contains("[providers.custom]"));
        assert!(once.contains("api_key_env = \"CUSTOM_API_KEY\""));
        let twice = set_provider(&once, "custom", "https://other.test/v1", "openai", false);
        assert_eq!(
            twice.matches("[providers.custom]").count(),
            1,
            "a second write replaces rather than duplicates"
        );
        assert!(twice.contains("https://other.test/v1"));
    }

    #[test]
    fn a_keyless_provider_writes_the_flag() {
        let out = set_provider("", "local", "http://localhost:8080/v1", "openai", true);
        assert!(out.contains("keyless = true"));
        assert!(!out.contains("api_key_env"));
    }

    #[test]
    fn the_environment_name_is_upper_snake_case() {
        assert_eq!(env_name("my-endpoint"), "MY_ENDPOINT_API_KEY");
        assert_eq!(env_name("groq"), "GROQ_API_KEY");
    }
}
