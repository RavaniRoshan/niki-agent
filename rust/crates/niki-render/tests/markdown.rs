use niki_render::{
    HeadingLevel, LineKind, MarkdownTheme, RenderedLine, display_width, render_markdown,
};

fn rendered(source: &str, width: usize) -> Vec<RenderedLine> {
    render_markdown(source, width, &MarkdownTheme::plain())
}

fn texts(lines: &[RenderedLine]) -> Vec<String> {
    lines.iter().map(RenderedLine::text).collect()
}

fn span_style<'a>(line: &'a RenderedLine, needle: &str) -> &'a niki_render::SpanStyle {
    for span in &line.spans {
        if span.text == needle {
            return &span.style;
        }
    }
    panic!("no span with text {needle:?} in {:?}", line.text());
}

#[test]
fn headings_of_each_level_are_distinct() {
    let theme = MarkdownTheme::plain();
    let mut styles = Vec::new();
    for depth in 1..=6 {
        let source = format!("{} Title {depth}", "#".repeat(depth));
        let lines = render_markdown(&source, 40, &theme);
        assert_eq!(lines.len(), 1, "level {depth}");
        let level = HeadingLevel::from_depth(depth).expect("depth in range");
        assert_eq!(lines[0].kind, LineKind::Heading(level));
        assert_eq!(lines[0].text(), format!("Title {depth}"));
        assert_eq!(lines[0].spans[0].style.heading, Some(level));
        styles.push(lines[0].spans[0].style);
    }
    for first in 0..styles.len() {
        for second in (first + 1)..styles.len() {
            assert_ne!(styles[first], styles[second], "heading styles must differ");
        }
    }
}

#[test]
fn nested_list_indent_is_kept() {
    let lines = rendered("- outer\n  - inner\n    - deepest", 40);
    assert_eq!(texts(&lines), ["- outer", "  - inner", "    - deepest"]);
    for line in &lines {
        assert_eq!(line.kind, LineKind::ListItem { ordered: false });
    }
}

#[test]
fn every_unordered_marker_starts_an_item() {
    let lines = rendered("- dash\n* star\n+ plus", 40);
    assert_eq!(texts(&lines), ["- dash", "- star", "- plus"]);
    for line in &lines {
        assert_eq!(line.kind, LineKind::ListItem { ordered: false });
    }
}

#[test]
fn ordered_list_numbering_stays_literal() {
    let lines = rendered("3. three\n7. seven", 40);
    assert_eq!(texts(&lines), ["3. three", "7. seven"]);
    for line in &lines {
        assert_eq!(line.kind, LineKind::ListItem { ordered: true });
    }
}

#[test]
fn list_continuation_lines_hang_under_the_text() {
    let lines = rendered("- alpha beta gamma delta", 12);
    assert_eq!(
        texts(&lines),
        ["- alpha beta", "  gamma", "  delta"],
        "continuations align with the item text"
    );
    for line in &lines {
        assert!(display_width(&line.text()) <= 12);
    }
}

#[test]
fn fenced_code_is_never_wrapped() {
    let source = "```\nlet answer = a_very_long_identifier_that_never_fits_the_width;\n```";
    let lines = rendered(source, 10);
    assert_eq!(lines.len(), 1, "the fences themselves are not drawn");
    assert_eq!(lines[0].kind, LineKind::Code);
    assert_eq!(
        lines[0].text(),
        "let answer = a_very_long_identifier_that_never_fits_the_width;"
    );
    assert!(lines[0].spans.iter().all(|span| span.style.code));
}

#[test]
fn tilde_fences_are_code_too() {
    let lines = rendered("~~~\nraw text\n~~~", 40);
    assert_eq!(texts(&lines), ["raw text"]);
    assert_eq!(lines[0].kind, LineKind::Code);
}

#[test]
fn inline_code_bold_italic_and_links_are_styled() {
    let source = "a `code` **bold** *it* __strong__ _em_ [site](https://example.com)";
    let lines = rendered(source, 200);
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert_eq!(
        line.text(),
        "a code bold it strong em site (https://example.com)"
    );
    assert!(span_style(line, "code").code);
    assert!(span_style(line, "bold").bold);
    assert!(span_style(line, "strong").bold);
    assert!(span_style(line, "it").italic);
    assert!(span_style(line, "em").italic);
    assert!(span_style(line, "site").link);
}

#[test]
fn blockquote_words_sit_behind_a_gutter() {
    let lines = rendered("> quoted\n> again", 40);
    assert_eq!(texts(&lines), ["│ quoted", "│ again"]);
    for line in &lines {
        assert_eq!(line.kind, LineKind::Quote);
        assert!(
            line.spans
                .iter()
                .any(|span| span.text.starts_with('│') && span.style.quote)
        );
    }
}

#[test]
fn a_rule_inside_a_fence_is_code() {
    let lines = rendered("---\n```\n---\n```\n***", 20);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].kind, LineKind::ThematicBreak);
    assert_eq!(lines[1].kind, LineKind::Code);
    assert_eq!(lines[1].text(), "---");
    assert_eq!(lines[2].kind, LineKind::ThematicBreak);
    assert_eq!(lines[2].spans[0].text, "─".repeat(20));
}

#[test]
fn unclosed_emphasis_markers_stay_literal() {
    let lines = rendered("**bold and *still", 200);
    assert_eq!(texts(&lines), ["**bold and *still"]);
    assert!(
        lines[0]
            .spans
            .iter()
            .all(|span| !span.style.bold && !span.style.italic),
        "an unclosed marker must not style the rest of the line"
    );

    let lines = rendered("a `code and [label](url", 200);
    assert_eq!(
        texts(&lines),
        ["a `code and [label](url"],
        "unclosed code and link markers keep every character"
    );
}

#[test]
fn a_fence_left_open_renders_the_rest_as_code() {
    let lines = rendered("```\ncode\nmore", 80);
    assert_eq!(texts(&lines), ["code", "more"]);
    for line in &lines {
        assert_eq!(line.kind, LineKind::Code);
    }
}

#[test]
fn paragraphs_wrap_at_the_width() {
    let source = "alpha beta gamma delta epsilon zeta eta theta";
    let lines = rendered(source, 20);
    assert!(lines.len() > 1, "the paragraph must wrap");
    for line in &lines {
        assert_eq!(line.kind, LineKind::Paragraph);
        assert!(display_width(&line.text()) <= 20, "{:?}", line.text());
    }
    assert_eq!(texts(&lines).join(" "), source);
}

#[test]
fn tabs_expand_to_four_spaces() {
    let lines = rendered("```\n\tif true {\n\t}\n```", 80);
    assert_eq!(texts(&lines), ["    if true {", "    }"]);
}

#[test]
fn empty_input_renders_no_lines() {
    assert!(render_markdown("", 80, &MarkdownTheme::plain()).is_empty());
}

#[test]
fn a_blank_between_a_paragraph_and_a_heading_survives() {
    let lines = rendered("paragraph\n\n# heading\n\n\ntail", 80);
    let kinds: Vec<LineKind> = lines.iter().map(|line| line.kind).collect();
    assert_eq!(
        kinds,
        [
            LineKind::Paragraph,
            LineKind::Blank,
            LineKind::Heading(HeadingLevel::H1),
            LineKind::Blank,
            LineKind::Paragraph,
        ],
        "one blank survives; a run of blanks collapses to one"
    );
    assert_eq!(
        texts(&lines),
        ["paragraph", "", "heading", "", "tail"],
        "a blank line is an empty line, not a missing one"
    );
}
