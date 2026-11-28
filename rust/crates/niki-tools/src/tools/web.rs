//! The two network tools: fetch a page, search the web.
//!
//! Both are read-only against the network and both declare
//! [`Tool::requires_approval`], so a person sees the first call before bytes
//! leave the machine — `Always` then covers the rest of the session. The search
//! goes to DuckDuckGo's HTML endpoint, which needs no key and no account; the
//! parser is deliberately small and reports honestly when the page shape
//! changes rather than inventing results.

use std::time::Duration;

use serde_json::{Value, json};

use crate::{Tool, ToolContext, ToolError, ToolOutput, args};

/// Largest page the fetch tool will read, before conversion.
const MAX_BODY: usize = 4 * 1024 * 1024;
/// Characters the fetch tool returns unless the caller asks for more.
const DEFAULT_CHARS: usize = 20_000;
/// Hard ceiling on requested characters, so one call cannot fill the context.
const MAX_CHARS: usize = 80_000;
/// Rows the search tool returns at most.
const MAX_RESULTS: usize = 10;

/// A shared client: connection setup costs more than the request.
fn client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("niki-agent/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

fn network(error: reqwest::Error) -> ToolError {
    ToolError::Network(error.to_string())
}

/// Fetch one URL and return its readable text.
pub struct WebFetch;

impl Tool for WebFetch {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch a URL over the network and return its readable text. Use it for documentation, release notes and pages a person links to."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The http:// or https:// URL to fetch."
                },
                "max_chars": {
                    "type": "integer",
                    "minimum": 100,
                    "description": "Characters to return. Defaults to 20000."
                }
            },
            "required": ["url"],
            "additionalProperties": false
        })
    }

    fn requires_approval(&self) -> bool {
        true
    }

    async fn run(&self, args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let url = args::required_str(&args, "url")?;
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return Err(ToolError::InvalidArguments(
                "web_fetch needs an http:// or https:// URL".to_owned(),
            ));
        }
        let limit = args::optional_u32(&args, "max_chars")?
            .map_or(DEFAULT_CHARS, |value| {
                usize::try_from(value).unwrap_or(DEFAULT_CHARS)
            })
            .clamp(100, MAX_CHARS);

        let response = client().get(url).send().await.map_err(network)?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        if let Some(length) = response.content_length() {
            if length > MAX_BODY as u64 {
                return Err(ToolError::InvalidArguments(format!(
                    "{url} is {length} bytes; web_fetch reads at most {MAX_BODY}"
                )));
            }
        }
        let body = response.text().await.map_err(network)?;
        let body = if body.len() > MAX_BODY {
            &body[..floor_char_boundary(&body, MAX_BODY)]
        } else {
            body.as_str()
        };
        if !status.is_success() {
            return Err(ToolError::Network(format!(
                "{url} answered {status}: {}",
                first_line(body)
            )));
        }

        let is_html = content_type.contains("html") || body.trim_start().starts_with('<');
        let text = if is_html {
            html_to_text(body)
        } else {
            body.to_owned()
        };
        let truncated = text.chars().count() > limit;
        let text: String = text.chars().take(limit).collect();
        let mut output = format!("{url}\n\n{text}");
        if truncated {
            output.push_str("\n\n[truncated; ask for more with max_chars]");
        }
        Ok(ToolOutput::new(output))
    }
}

/// Search the web through DuckDuckGo's keyless HTML endpoint.
pub struct WebSearch;

impl Tool for WebSearch {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web and return result titles, URLs and snippets. Use it when the answer is not in the workspace and the starting URL is unknown."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "What to search for."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 10,
                    "description": "Results to return. Defaults to 5."
                }
            },
            "required": ["query"],
            "additionalProperties": false
        })
    }

    fn requires_approval(&self) -> bool {
        true
    }

    async fn run(&self, args: Value, _ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let query = args::required_str(&args, "query")?;
        if query.trim().is_empty() {
            return Err(ToolError::InvalidArguments(
                "web_search needs a query".to_owned(),
            ));
        }
        let limit = args::optional_u32(&args, "limit")?
            .map_or(5, |value| usize::try_from(value).unwrap_or(5))
            .clamp(1, MAX_RESULTS);

        let url = format!(
            "https://html.duckduckgo.com/html/?q={}",
            percent_encode(query.trim())
        );
        let response = client().get(&url).send().await.map_err(network)?;
        let status = response.status();
        let body = response.text().await.map_err(network)?;
        if !status.is_success() {
            return Err(ToolError::Network(format!(
                "the search endpoint answered {status}"
            )));
        }

        let results = parse_results(&body, limit);
        if results.is_empty() {
            return Ok(ToolOutput::new(format!(
                "no results parsed for `{}`; the search endpoint may have changed shape",
                query.trim()
            )));
        }
        let mut out = format!("search: {}\n", query.trim());
        for (index, result) in results.iter().enumerate() {
            out.push_str(&format!(
                "\n{}. {}\n   {}\n",
                index + 1,
                result.title,
                result.url
            ));
            if !result.snippet.is_empty() {
                out.push_str(&format!("   {}\n", result.snippet));
            }
        }
        Ok(ToolOutput::new(out))
    }
}

/// One parsed search row.
#[derive(Debug, PartialEq, Eq)]
struct SearchResult {
    title: String,
    url: String,
    snippet: String,
}

/// Pull result anchors out of DuckDuckGo's HTML page.
fn parse_results(html: &str, limit: usize) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let mut rest = html;
    while results.len() < limit {
        let Some(start) = rest.find("class=\"result__a\"") else {
            break;
        };
        let after = &rest[start..];
        let Some(href_start) = after.find("href=\"") else {
            break;
        };
        let href = &after[href_start + 6..];
        let Some(href_end) = href.find('"') else {
            break;
        };
        let link = &href[..href_end];
        let Some(title_start) = href[href_end..].find('>') else {
            break;
        };
        let title_body = &href[href_end + title_start + 1..];
        let Some(title_end) = title_body.find("</a>") else {
            break;
        };
        let title = decode_entities(&strip_tags(&title_body[..title_end]));

        let next = &title_body[title_end..];
        let snippet = match next.find("class=\"result__snippet\"") {
            Some(at) => {
                let snippet_body = &next[at..];
                snippet_body
                    .find('>')
                    .and_then(|open| {
                        let body = &snippet_body[open + 1..];
                        body.find("</a>")
                            .map(|end| collapse(&decode_entities(&strip_tags(&body[..end]))))
                    })
                    .unwrap_or_default()
            }
            None => String::new(),
        };

        results.push(SearchResult {
            title: collapse(&title),
            url: unwrap_redirect(link),
            snippet,
        });
        rest = &title_body[title_end..];
    }
    results
}

/// Turn a DuckDuckGo redirect into the destination it names.
fn unwrap_redirect(link: &str) -> String {
    if let Some(at) = link.find("uddg=") {
        let value = &link[at + 5..];
        let value = value.split('&').next().unwrap_or(value);
        let decoded = percent_decode(value);
        if decoded.starts_with("http") {
            return decoded;
        }
    }
    if link.starts_with("//") {
        return format!("https:{link}");
    }
    link.to_owned()
}

/// Reduce an HTML page to its readable text.
fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 3);
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let Some(close) = after.find('>') else {
            break;
        };
        let tag = &after[1..close];
        let lower = tag.to_ascii_lowercase();
        if lower.starts_with("script") || lower.starts_with("style") {
            let end_tag = format!(
                "</{}",
                if lower.starts_with("script") {
                    "script"
                } else {
                    "style"
                }
            );
            match rest[at..].to_ascii_lowercase().find(&end_tag) {
                Some(end) => {
                    rest = &rest[at + end..];
                    let skip = rest.find('>').map_or(rest.len(), |value| value + 1);
                    rest = &rest[skip..];
                    continue;
                }
                None => break,
            }
        }
        if matches!(
            lower.split_whitespace().next().unwrap_or_default(),
            "p" | "br" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        ) || lower.starts_with("/p")
            || lower.starts_with("/div")
            || lower.starts_with("/li")
        {
            out.push('\n');
        } else if lower.starts_with("td") || lower.starts_with("/td") {
            out.push(' ');
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    collapse(&decode_entities(&out))
}

/// Drop every tag from a fragment, keeping its text.
fn strip_tags(fragment: &str) -> String {
    let mut out = String::with_capacity(fragment.len());
    let mut rest = fragment;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        match rest[at..].find('>') {
            Some(close) => rest = &rest[at + close + 1..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Join runs of whitespace into one space and trim the ends.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The handful of entities a result page actually uses.
fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let Some(end) = after.find(';').filter(|end| *end <= 10) else {
            out.push('&');
            rest = &after[1..];
            continue;
        };
        let entity = &after[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| {
                    entity
                        .strip_prefix('#')
                        .and_then(|digits| digits.parse::<u32>().ok())
                })
                .and_then(char::from_u32),
        };
        match decoded {
            Some(character) => {
                out.push(character);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Percent-encode a query, leaving only the unreserved characters.
fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Decode `%XX` escapes, keeping whole UTF-8 sequences.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
            if let Some(value) = hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The last character boundary at or before `index`.
fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

/// The first line of a body, for an error message.
fn first_line(body: &str) -> String {
    let line = body.lines().next().unwrap_or_default().trim();
    line.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_becomes_readable_text() {
        let html = "<html><head><style>p{color:red}</style><script>var x=1;</script></head>\
                    <body><h1>Title</h1><p>First &amp; second</p><p>Third</p></body></html>";
        let text = html_to_text(html);
        assert_eq!(text, "Title First & second Third");
    }

    #[test]
    fn entities_decode() {
        assert_eq!(
            decode_entities("a &amp; b &lt;c&gt; &#39;d&#39;"),
            "a & b <c> 'd'"
        );
        assert_eq!(
            decode_entities("100% &unknown; here"),
            "100% &unknown; here"
        );
    }

    #[test]
    fn a_query_encodes_spaces_and_symbols() {
        assert_eq!(percent_encode("rust tui"), "rust+tui");
        assert_eq!(percent_encode("a/b?c"), "a%2Fb%3Fc");
    }

    #[test]
    fn a_redirect_unwraps_to_its_destination() {
        assert_eq!(
            unwrap_redirect("//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.test%2Fx&rut=1"),
            "https://example.test/x"
        );
        assert_eq!(
            unwrap_redirect("https://direct.test"),
            "https://direct.test"
        );
    }

    #[test]
    fn a_result_page_parses_titles_urls_and_snippets() {
        let html = r##"
          <div class="result">
            <a rel="nofollow" class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.test%2Fone&amp;rut=x">One &amp; only</a>
            <a class="result__snippet" href="#">The <b>first</b> snippet.</a>
          </div>
          <div class="result">
            <a rel="nofollow" class="result__a" href="https://two.test/page">Two</a>
            <a class="result__snippet">Second one.</a>
          </div>"##;
        let results = parse_results(html, 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "One & only");
        assert_eq!(results[0].url, "https://example.test/one");
        assert_eq!(results[0].snippet, "The first snippet.");
        assert_eq!(results[1].url, "https://two.test/page");
        assert_eq!(results[1].snippet, "Second one.");
    }

    #[test]
    fn the_limit_is_respected() {
        let html = (0..5)
            .map(|index| {
                format!(r#"<a class="result__a" href="https://x.test/{index}">T{index}</a>"#)
            })
            .collect::<String>();
        assert_eq!(parse_results(&html, 3).len(), 3);
    }
}
