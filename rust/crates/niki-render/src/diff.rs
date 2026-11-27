//! Unified diff parsing and rendering into styled lines.
//!
//! [`parse_unified`] reads the `---`/`+++` file header and every `@@` hunk of
//! one file, numbering its lines the way `git diff` does: a context line
//! advances both counters, a removal only the old one and an addition only the
//! new one. `\ No newline at end of file` is not a hunk line and consumes no
//! number. Text with no hunk header returns `None`, so a caller can fall back
//! to plain text.
//!
//! [`render_diff`] draws the hunks with the same [`RenderedLine`] vocabulary as
//! the Markdown renderer, so a terminal paints both through one code path. A
//! line is an old-number/new-number gutter, a `│` divider, and the sign plus
//! text. A line wider than the requested width is truncated rather than
//! wrapped: wrapping would break the alignment the numbering exists to give.

use super::{LineKind, RenderedLine, Rgb, Span, SpanStyle, display_width, unicode_width};

/// What a diff line does to the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKind {
    /// A line both sides keep.
    Context,
    /// A line only the new side has.
    Add,
    /// A line only the old side has.
    Remove,
}

/// One line of a hunk, with the numbers its side counts to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// Whether the line is context, added, or removed.
    pub kind: DiffKind,
    /// The old file's line number; `None` for an addition.
    pub old_no: Option<u32>,
    /// The new file's line number; `None` for a removal.
    pub new_no: Option<u32>,
    /// The line without its leading sign.
    pub text: String,
}

/// One `@@` hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// The old file's first line in this hunk; `0` when the file is new.
    pub old_start: u32,
    /// The new file's first line in this hunk; `0` when the file is deleted.
    pub new_start: u32,
    /// The hunk body, in file order.
    pub lines: Vec<DiffLine>,
}

/// One file's diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// The path both sides name, without the `a/`/`b/` prefix.
    pub path: String,
    /// The hunks, in the order the input listed them.
    pub hunks: Vec<Hunk>,
}

/// The palette a diff is drawn with.
///
/// The fields are plain sRGB values rather than palette tokens, because this
/// crate holds no terminal or palette dependency. [`Default`] mirrors the
/// default palette's `ok`, `error`, `accent`, and `muted` tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffTheme {
    /// Added lines.
    pub add: Rgb,
    /// Removed lines.
    pub remove: Rgb,
    /// A `@@` hunk header.
    pub hunk: Rgb,
    /// The old/new number gutter and its divider.
    pub gutter: Rgb,
}

impl DiffTheme {
    /// Build a theme from four colours.
    #[must_use]
    pub const fn new(add: Rgb, remove: Rgb, hunk: Rgb, gutter: Rgb) -> Self {
        Self {
            add,
            remove,
            hunk,
            gutter,
        }
    }
}

impl Default for DiffTheme {
    fn default() -> Self {
        Self {
            add: Rgb::new(0xa8, 0xe6, 0xa1),
            remove: Rgb::new(0xff, 0x9d, 0x9d),
            hunk: Rgb::new(0x8e, 0xc7, 0xff),
            gutter: Rgb::new(0xaa, 0xb0, 0xbd),
        }
    }
}

/// Parse one file's unified diff.
///
/// Returns `None` when the text holds no `@@` hunk header. When the input holds
/// several files, only the first is returned; the path comes from the `+++`
/// side, falling back to the `---` side when the new side is `/dev/null`.
#[must_use]
pub fn parse_unified(text: &str) -> Option<FileDiff> {
    let lines: Vec<&str> = text.lines().collect();
    let mut path = String::new();
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        if let Some(header) = hunk_header(lines[index]) {
            let mut hunk = Hunk {
                old_start: header.old_start,
                new_start: header.new_start,
                lines: Vec::new(),
            };
            let mut old_no = header.old_start;
            let mut new_no = header.new_start;
            let mut old_left = header.old_count;
            let mut new_left = header.new_count;
            index += 1;

            while index < lines.len() {
                let line = lines[index];
                if hunk_header(line).is_some() {
                    break;
                }
                if is_no_newline_marker(line) {
                    index += 1;
                    continue;
                }
                // Counts bound the hunk, which keeps a `---`/`+++` pair inside
                // the body from being read as the next file's header.
                let done =
                    old_left.is_none_or(|left| left == 0) && new_left.is_none_or(|left| left == 0);
                if done && file_header(&lines, index).is_some() {
                    break;
                }
                let Some((kind, content)) = diff_line(line) else {
                    break;
                };
                let (old, new) = match kind {
                    DiffKind::Context => {
                        let numbers = (Some(old_no), Some(new_no));
                        old_no = old_no.saturating_add(1);
                        new_no = new_no.saturating_add(1);
                        numbers
                    }
                    DiffKind::Remove => {
                        let numbers = (Some(old_no), None);
                        old_no = old_no.saturating_add(1);
                        numbers
                    }
                    DiffKind::Add => {
                        let numbers = (None, Some(new_no));
                        new_no = new_no.saturating_add(1);
                        numbers
                    }
                };
                if kind != DiffKind::Add {
                    consume(&mut old_left);
                }
                if kind != DiffKind::Remove {
                    consume(&mut new_left);
                }
                hunk.lines.push(DiffLine {
                    kind,
                    old_no: old,
                    new_no: new,
                    text: content.to_owned(),
                });
                index += 1;
            }
            hunks.push(hunk);
            continue;
        }

        if let Some((old, new)) = file_header(&lines, index) {
            if !hunks.is_empty() {
                break;
            }
            path = file_path(old, new);
            index += 2;
            continue;
        }

        // A hand-written diff may name only one side of the file header.
        if hunks.is_empty() && path.is_empty() {
            if let Some(target) = header_target(lines[index], "+++ ") {
                if target != "/dev/null" {
                    path = target.to_owned();
                }
                index += 1;
                continue;
            }
            if let Some(target) = header_target(lines[index], "--- ") {
                if target != "/dev/null" {
                    path = target.to_owned();
                }
                index += 1;
                continue;
            }
        }

        index += 1;
    }

    if hunks.is_empty() {
        return None;
    }
    Some(FileDiff { path, hunks })
}

/// Render a diff into styled lines no wider than `width`.
///
/// Each hunk is a header line followed by its body; every body line starts with
/// an old/new number gutter. Lines wider than `width` are truncated.
#[must_use]
pub fn render_diff(diff: &FileDiff, width: usize, theme: &DiffTheme) -> Vec<RenderedLine> {
    let width = width.max(1);
    let mut old_width = 1;
    let mut new_width = 1;
    for line in diff.hunks.iter().flat_map(|hunk| &hunk.lines) {
        if let Some(number) = line.old_no {
            old_width = old_width.max(number_width(number));
        }
        if let Some(number) = line.new_no {
            new_width = new_width.max(number_width(number));
        }
    }

    let mut rendered = Vec::new();
    for hunk in &diff.hunks {
        let old_count = hunk
            .lines
            .iter()
            .filter(|line| line.kind != DiffKind::Add)
            .count();
        let new_count = hunk
            .lines
            .iter()
            .filter(|line| line.kind != DiffKind::Remove)
            .count();
        let header = format!(
            "@@ -{},{} +{},{} @@",
            hunk.old_start, old_count, hunk.new_start, new_count
        );
        rendered.push(RenderedLine {
            spans: truncate(
                vec![Span {
                    text: header,
                    style: SpanStyle::PLAIN.with_color(theme.hunk),
                }],
                width,
            ),
            kind: LineKind::DiffHunk,
        });

        for line in &hunk.lines {
            let gutter = format!(
                "{} {} │",
                gutter_number(line.old_no, old_width),
                gutter_number(line.new_no, new_width)
            );
            let (sign, style) = match line.kind {
                DiffKind::Context => (' ', SpanStyle::PLAIN),
                DiffKind::Add => ('+', SpanStyle::PLAIN.with_color(theme.add)),
                DiffKind::Remove => ('-', SpanStyle::PLAIN.with_color(theme.remove)),
            };
            rendered.push(RenderedLine {
                spans: truncate(
                    vec![
                        Span {
                            text: gutter,
                            style: SpanStyle::PLAIN.with_color(theme.gutter),
                        },
                        Span {
                            text: format!("{sign}{}", line.text),
                            style,
                        },
                    ],
                    width,
                ),
                kind: LineKind::Diff(line.kind),
            });
        }
    }
    rendered
}

fn gutter_number(number: Option<u32>, width: usize) -> String {
    match number {
        Some(number) => format!("{number:>width$}"),
        None => " ".repeat(width),
    }
}

fn number_width(value: u32) -> usize {
    value
        .checked_ilog10()
        .map_or(1, |digits| digits as usize + 1)
}

fn truncate(spans: Vec<Span>, width: usize) -> Vec<Span> {
    let mut kept = Vec::new();
    let mut used = 0;
    for span in spans {
        if used >= width {
            break;
        }
        let room = width - used;
        let span_width = display_width(&span.text);
        if span_width <= room {
            used += span_width;
            kept.push(span);
            continue;
        }
        let text = clip(&span.text, room);
        if !text.is_empty() {
            kept.push(Span {
                text,
                style: span.style,
            });
        }
        break;
    }
    kept
}

fn clip(text: &str, width: usize) -> String {
    let mut clipped = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let cells = unicode_width(ch);
        if used + cells > width {
            break;
        }
        clipped.push(ch);
        used += cells;
    }
    clipped
}

fn consume(counter: &mut Option<u32>) {
    if let Some(value) = counter {
        *value = value.saturating_sub(1);
    }
}

/// The two header lines of a file, when `index` names a `---`/`+++` pair.
fn file_header<'a>(lines: &[&'a str], index: usize) -> Option<(&'a str, &'a str)> {
    let old = *lines.get(index)?;
    let new = *lines.get(index + 1)?;
    if old.starts_with("--- ") && new.starts_with("+++ ") {
        Some((old, new))
    } else {
        None
    }
}

fn file_path(old: &str, new: &str) -> String {
    let old = header_target(old, "--- ");
    let new = header_target(new, "+++ ");
    new.filter(|path| *path != "/dev/null")
        .or(old.filter(|path| *path != "/dev/null"))
        .unwrap_or_default()
        .to_owned()
}

fn header_target<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(prefix)?;
    let rest = rest.split('\t').next().unwrap_or(rest).trim();
    let rest = rest
        .strip_prefix("a/")
        .or_else(|| rest.strip_prefix("b/"))
        .unwrap_or(rest);
    Some(rest)
}

fn hunk_header(line: &str) -> Option<HunkHeader> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(' ')?;
    let rest = rest.strip_prefix('+')?;
    let (new, tail) = rest.split_once(' ')?;
    if !tail.starts_with("@@") {
        return None;
    }
    let (old_start, old_count) = header_side(old)?;
    let (new_start, new_count) = header_side(new)?;
    Some(HunkHeader {
        old_start,
        old_count,
        new_start,
        new_count,
    })
}

#[derive(Debug, Clone, Copy)]
struct HunkHeader {
    old_start: u32,
    old_count: Option<u32>,
    new_start: u32,
    new_count: Option<u32>,
}

fn header_side(spec: &str) -> Option<(u32, Option<u32>)> {
    match spec.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, Some(count.parse().ok()?))),
        None => Some((spec.parse().ok()?, None)),
    }
}

fn diff_line(line: &str) -> Option<(DiffKind, &str)> {
    if let Some(text) = line.strip_prefix(' ') {
        return Some((DiffKind::Context, text));
    }
    if let Some(text) = line.strip_prefix('+') {
        return Some((DiffKind::Add, text));
    }
    if let Some(text) = line.strip_prefix('-') {
        return Some((DiffKind::Remove, text));
    }
    if line.is_empty() {
        return Some((DiffKind::Context, ""));
    }
    None
}

fn is_no_newline_marker(line: &str) -> bool {
    line.starts_with("\\ No newline at end of file")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_width;

    const TWO_HUNKS: &str = "\
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@
 fn main() {
-    old();
+    new();
 }
@@ -10,2 +10,3 @@ context heading
 context
+added
 tail
";

    fn numbered(hunk: &Hunk) -> Vec<(DiffKind, Option<u32>, Option<u32>, &str)> {
        hunk.lines
            .iter()
            .map(|line| (line.kind, line.old_no, line.new_no, line.text.as_str()))
            .collect()
    }

    #[test]
    fn two_hunks_number_every_line() {
        let diff = parse_unified(TWO_HUNKS).expect("a unified diff");
        assert_eq!(diff.path, "src/main.rs");
        assert_eq!(diff.hunks.len(), 2);

        assert_eq!(diff.hunks[0].old_start, 1);
        assert_eq!(diff.hunks[0].new_start, 1);
        assert_eq!(
            numbered(&diff.hunks[0]),
            [
                (DiffKind::Context, Some(1), Some(1), "fn main() {"),
                (DiffKind::Remove, Some(2), None, "    old();"),
                (DiffKind::Add, None, Some(2), "    new();"),
                (DiffKind::Context, Some(3), Some(3), "}"),
            ]
        );

        assert_eq!(diff.hunks[1].old_start, 10);
        assert_eq!(diff.hunks[1].new_start, 10);
        assert_eq!(
            numbered(&diff.hunks[1]),
            [
                (DiffKind::Context, Some(10), Some(10), "context"),
                (DiffKind::Add, None, Some(11), "added"),
                (DiffKind::Context, Some(11), Some(12), "tail"),
            ]
        );

        let theme = DiffTheme::default();
        let rendered = render_diff(&diff, 80, &theme);
        assert_eq!(rendered.len(), 9);
        assert_eq!(rendered[0].kind, LineKind::DiffHunk);
        assert_eq!(rendered[0].text(), "@@ -1,3 +1,3 @@");
        assert_eq!(rendered[1].kind, LineKind::Diff(DiffKind::Context));
        assert_eq!(rendered[2].kind, LineKind::Diff(DiffKind::Remove));
        assert_eq!(rendered[3].kind, LineKind::Diff(DiffKind::Add));
        assert!(rendered[1].text().ends_with("│ fn main() {"));
        assert!(rendered[2].text().ends_with("│-    old();"));
        assert!(rendered[3].text().ends_with("│+    new();"));
        assert_eq!(
            rendered[5].text(),
            "@@ -10,2 +10,3 @@",
            "the second hunk header is reconstructed from its lines"
        );
        assert_eq!(
            rendered[3].spans.last().expect("text span").style.color,
            Some(theme.add)
        );
        assert_eq!(
            rendered[2].spans.last().expect("text span").style.color,
            Some(theme.remove)
        );
        assert_eq!(
            rendered[3].spans[0].style.color,
            Some(theme.gutter),
            "the gutter carries the gutter colour"
        );
    }

    #[test]
    fn an_added_file_has_no_old_numbers() {
        let diff =
            parse_unified("--- /dev/null\n+++ b/notes/new.txt\n@@ -0,0 +1,2 @@\n+first\n+second\n")
                .expect("a unified diff");
        assert_eq!(diff.path, "notes/new.txt");
        assert_eq!(diff.hunks[0].old_start, 0);
        assert_eq!(
            numbered(&diff.hunks[0]),
            [
                (DiffKind::Add, None, Some(1), "first"),
                (DiffKind::Add, None, Some(2), "second"),
            ]
        );
        let rendered = render_diff(&diff, 80, &DiffTheme::default());
        assert_eq!(rendered[0].text(), "@@ -0,0 +1,2 @@");
    }

    #[test]
    fn an_added_file_without_an_old_header_still_parses() {
        let diff =
            parse_unified("+++ b/fresh.txt\n@@ -0,0 +1,1 @@\n+hi\n").expect("a unified diff");
        assert_eq!(diff.path, "fresh.txt");
        assert_eq!(
            numbered(&diff.hunks[0]),
            [(DiffKind::Add, None, Some(1), "hi")]
        );
    }

    #[test]
    fn a_deleted_file_has_no_new_numbers() {
        let diff =
            parse_unified("--- a/old.txt\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-gone\n-also gone\n")
                .expect("a unified diff");
        assert_eq!(diff.path, "old.txt");
        assert_eq!(diff.hunks[0].new_start, 0);
        assert_eq!(
            numbered(&diff.hunks[0]),
            [
                (DiffKind::Remove, Some(1), None, "gone"),
                (DiffKind::Remove, Some(2), None, "also gone"),
            ]
        );
        let rendered = render_diff(&diff, 80, &DiffTheme::default());
        assert_eq!(rendered[0].text(), "@@ -1,2 +0,0 @@");
    }

    #[test]
    fn the_no_newline_marker_consumes_no_number() {
        let diff = parse_unified(
            r"@@ -1,1 +1,1 @@
-old
\ No newline at end of file
+new
\ No newline at end of file
",
        )
        .expect("a unified diff");
        assert_eq!(
            numbered(&diff.hunks[0]),
            [
                (DiffKind::Remove, Some(1), None, "old"),
                (DiffKind::Add, None, Some(1), "new"),
            ],
            "the marker is not a hunk line"
        );
    }

    #[test]
    fn a_long_line_is_truncated_not_wrapped() {
        let diff = FileDiff {
            path: "big.txt".to_owned(),
            hunks: vec![Hunk {
                old_start: 1,
                new_start: 1,
                lines: vec![DiffLine {
                    kind: DiffKind::Add,
                    old_no: None,
                    new_no: Some(1),
                    text: "a".repeat(200),
                }],
            }],
        };
        let rendered = render_diff(&diff, 20, &DiffTheme::default());
        assert_eq!(rendered.len(), 2, "one header and one body line");
        for line in &rendered {
            assert!(display_width(&line.text()) <= 20);
        }
        assert_eq!(rendered[1].text(), format!("  1 │+{}", "a".repeat(14)));
    }

    #[test]
    fn text_without_hunks_is_not_a_diff() {
        assert!(parse_unified("").is_none());
        assert!(parse_unified("hello\nworld\n").is_none());
        assert!(parse_unified("--- a/x\n+++ b/x\n").is_none());
    }
}
