//! Markdown structure and byte-accurate spans.
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Byte range and one-based line and column in the original source.
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
/// Parsed Markdown heading and its original source span.
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
/// Parsed Markdown link destination and original source span.
pub struct Link {
    pub destination: String,
    pub span: Span,
}
#[derive(Debug, Clone, PartialEq, Eq)]
/// Fenced code block information and original source span.
pub struct Fence {
    pub info: String,
    pub span: Span,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
/// Markdown structures extracted from an instruction document.
pub struct ParsedDocument {
    pub headings: Vec<Heading>,
    pub list_items: Vec<Span>,
    pub links: Vec<Link>,
    pub fences: Vec<Fence>,
    pub frontmatter: Option<Span>,
}
fn span(source: &str, line_starts: &[usize], range: std::ops::Range<usize>) -> Span {
    let line_index = line_starts.partition_point(|&start| start <= range.start) - 1;
    let prefix = &source[line_starts[line_index]..range.start];
    Span {
        start: range.start,
        end: range.end,
        line: line_index + 1,
        column: prefix.chars().count() + 1,
    }
}
/// Parse Markdown without executing input. Spans include complete raw constructs.
#[must_use]
pub fn parse(source: &str) -> ParsedDocument {
    let mut result = ParsedDocument::default();
    let mut line_starts = vec![0];
    line_starts.extend(source.match_indices('\n').map(|(index, _)| index + 1));
    let mut heading: Option<(u8, String, Span)> = None;
    let markdown = source.strip_prefix('\u{feff}').unwrap_or(source);
    let offset = source.len() - markdown.len();
    for (event, range) in Parser::new(markdown).into_offset_iter() {
        let range = range.start + offset..range.end + offset;
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some((
                    level as u8,
                    String::new(),
                    span(source, &line_starts, range),
                ));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text, position)) = heading.take() {
                    result.headings.push(Heading {
                        level,
                        text,
                        span: position,
                    });
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, title, _)) = &mut heading {
                    title.push_str(&text);
                }
            }
            Event::Start(Tag::Item) => result.list_items.push(span(source, &line_starts, range)),
            Event::Start(Tag::Link { dest_url, .. }) => result.links.push(Link {
                destination: dest_url.to_string(),
                span: span(source, &line_starts, range),
            }),
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                result.fences.push(Fence {
                    info: info.to_string(),
                    span: span(source, &line_starts, range),
                });
            }
            _ => {}
        }
    }
    let frontmatter_source = source.strip_prefix('\u{feff}').unwrap_or(source);
    if frontmatter_source.starts_with("---\n") || frontmatter_source.starts_with("---\r\n") {
        let mut offset = source.find('\n').map_or(source.len(), |n| n + 1);
        for line in source[offset..].split_inclusive('\n') {
            offset += line.len();
            if matches!(line.trim_end_matches(['\r', '\n']), "---" | "...") {
                result.frontmatter = Some(span(source, &line_starts, 0..offset));
                break;
            }
        }
    }
    if let Some(frontmatter) = result.frontmatter {
        result
            .headings
            .retain(|item| item.span.start >= frontmatter.end);
        result
            .list_items
            .retain(|item| item.start >= frontmatter.end);
        result
            .links
            .retain(|item| item.span.start >= frontmatter.end);
        result
            .fences
            .retain(|item| item.span.start >= frontmatter.end);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bom_and_yaml_end_marker_form_frontmatter() {
        let text = "\u{feff}---\r\nname: test\r\n...\r\n# Title\n";
        let parsed = parse(text);
        let fm = parsed.frontmatter.unwrap();
        assert_eq!(
            &text[fm.start..fm.end],
            "\u{feff}---\r\nname: test\r\n...\r\n"
        );
        assert_eq!(parsed.headings.len(), 1);
        assert_eq!(parsed.headings[0].text, "Title");
    }
    #[test]
    fn structures_have_raw_spans() {
        let text = "---\nname: x\n---\n# Hi\n\n- item\n\n```sh\nls\n```\n";
        let p = parse(text);
        assert_eq!(p.headings[0].text, "Hi");
        assert!(text[p.list_items[0].start..p.list_items[0].end].starts_with("- item"));
        assert!(text[p.fences[0].span.start..p.fences[0].span.end].starts_with("```sh"));
        assert_eq!(p.frontmatter.unwrap().start, 0);
        assert_eq!(p.headings.len(), 1, "frontmatter is not a heading");
    }
}
