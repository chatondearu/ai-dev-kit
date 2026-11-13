//! Terminal-oriented markdown rendering (pulldown-cmark).
//!
//! Shared by `cdamux notes show --render` and the Fleet Foam sidecar preview.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Flatten markdown to a plain terminal string (headings, lists, code spans).
///
/// No HTML, images, or `[[wikilink]]` resolution — intentional v0 limits.
pub fn render_markdown_terminal(md: &str) -> String {
    let parser = Parser::new_ext(md, Options::all());
    let mut out = String::new();
    let mut heading_prefix = false;
    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                out.push('\n');
                for _ in 0..level as usize {
                    out.push('#');
                }
                out.push(' ');
                heading_prefix = true;
            }
            Event::End(TagEnd::Heading(_)) => {
                out.push('\n');
                heading_prefix = false;
            }
            Event::Start(Tag::List(_)) => out.push('\n'),
            Event::Start(Tag::Item) => out.push_str("- "),
            Event::End(TagEnd::Item) => out.push('\n'),
            Event::Text(t) => out.push_str(&t),
            Event::Code(t) => {
                out.push('`');
                out.push_str(&t);
                out.push('`');
            }
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push('\n'),
            Event::Rule => out.push_str("---\n"),
            _ => {}
        }
    }
    if heading_prefix {
        out.push('\n');
    }
    out
}

/// Keep at most `max_lines` lines; append a visible cue when truncated.
pub fn truncate_lines(body: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() <= max_lines {
        return body.to_string();
    }
    let mut out = lines[..max_lines].join("\n");
    out.push_str("\n… (truncated)");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_heading_and_list() {
        let out = render_markdown_terminal("# Title\n\n- one\n- two\n");
        assert!(out.contains("# Title"), "{out}");
        assert!(out.contains("- one"), "{out}");
        assert!(out.contains("- two"), "{out}");
    }

    #[test]
    fn truncate_lines_adds_cue() {
        let body = (0..5).map(|i| format!("L{i}")).collect::<Vec<_>>().join("\n");
        let out = truncate_lines(&body, 3);
        assert_eq!(out, "L0\nL1\nL2\n… (truncated)");
        assert_eq!(truncate_lines(&body, 10), body);
    }
}
