use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, Default)]
pub(crate) struct MarkdownSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
    pub link: Option<String>,
}
#[derive(Debug, Clone)]
pub(crate) enum MarkdownBlock {
    Heading {
        level: u8,
        anchor: String,
        spans: Vec<MarkdownSpan>,
    },
    Text {
        marker: Option<String>,
        depth: usize,
        quote: bool,
        spans: Vec<MarkdownSpan>,
    },
    Code(String),
    Rule,
}
impl MarkdownBlock {
    #[cfg(test)]
    pub fn spans(&self) -> &[MarkdownSpan] {
        match self {
            Self::Heading { spans, .. } | Self::Text { spans, .. } => spans,
            _ => &[],
        }
    }
}
pub(crate) fn heading_anchor(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                Some(c)
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn parse_markdown(source: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut spans = Vec::new();
    let mut heading = None;
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut marker: Option<String> = None;
    let mut quote = 0usize;
    let mut bold = 0usize;
    let mut italic = 0usize;
    let mut links: Vec<String> = Vec::new();
    let mut code: Option<String> = None;
    let flush = |blocks: &mut Vec<MarkdownBlock>,
                 spans: &mut Vec<MarkdownSpan>,
                 heading: Option<u8>,
                 marker: &mut Option<String>,
                 depth: usize,
                 quote: usize| {
        if spans.is_empty() {
            return;
        }
        let text = std::mem::take(spans);
        blocks.push(if let Some(level) = heading {
            let plain: String = text.iter().map(|s| s.text.as_str()).collect();
            MarkdownBlock::Heading {
                level,
                anchor: heading_anchor(&plain),
                spans: text,
            }
        } else {
            MarkdownBlock::Text {
                marker: marker.take(),
                depth,
                quote: quote > 0,
                spans: text,
            }
        });
    };
    for event in Parser::new_ext(
        source,
        Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH,
    ) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some(level as u8);
            }
            Event::End(TagEnd::Heading(_)) => {
                flush(
                    &mut blocks,
                    &mut spans,
                    heading.take(),
                    &mut marker,
                    lists.len(),
                    quote,
                );
            }
            Event::Start(Tag::CodeBlock(_)) => {
                code = Some(String::new());
            }
            Event::End(TagEnd::CodeBlock) => {
                blocks.push(MarkdownBlock::Code(code.take().unwrap_or_default()));
            }
            Event::Start(Tag::List(start)) => {
                flush(
                    &mut blocks,
                    &mut spans,
                    None,
                    &mut marker,
                    lists.len(),
                    quote,
                );
                lists.push(start);
            }
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                marker = Some(if let Some(Some(number)) = lists.last_mut() {
                    let label = format!("{number}.");
                    *number += 1;
                    label
                } else {
                    "•".into()
                });
            }
            Event::End(TagEnd::Item | TagEnd::Paragraph) => {
                flush(
                    &mut blocks,
                    &mut spans,
                    None,
                    &mut marker,
                    lists.len(),
                    quote,
                );
            }
            Event::Start(Tag::BlockQuote(_)) => {
                quote += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                quote = quote.saturating_sub(1);
            }
            Event::Start(Tag::Strong) => {
                bold += 1;
            }
            Event::End(TagEnd::Strong) => {
                bold = bold.saturating_sub(1);
            }
            Event::Start(Tag::Emphasis) => {
                italic += 1;
            }
            Event::End(TagEnd::Emphasis) => {
                italic = italic.saturating_sub(1);
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                links.push(dest_url.into_string());
            }
            Event::End(TagEnd::Link) => {
                links.pop();
            }
            Event::Text(text) | Event::Code(text) if code.is_some() => {
                code.as_mut().unwrap().push_str(&text);
            }
            Event::Text(text) => spans.push(MarkdownSpan {
                text: text.into_string(),
                bold: bold > 0,
                italic: italic > 0,
                link: links.last().cloned(),
                ..Default::default()
            }),
            Event::Code(text) => spans.push(MarkdownSpan {
                text: text.into_string(),
                code: true,
                bold: bold > 0,
                italic: italic > 0,
                link: links.last().cloned(),
            }),
            Event::SoftBreak | Event::HardBreak => {
                spans.push(MarkdownSpan {
                    text: if matches!(event, Event::HardBreak) {
                        "\n"
                    } else {
                        " "
                    }
                    .into(),
                    ..Default::default()
                });
            }
            Event::TaskListMarker(done) => spans.push(MarkdownSpan {
                text: if done { "[x] " } else { "[ ] " }.into(),
                ..Default::default()
            }),
            Event::Rule => {
                blocks.push(MarkdownBlock::Rule);
            }
            _ => {}
        }
    }
    flush(
        &mut blocks,
        &mut spans,
        heading,
        &mut marker,
        lists.len(),
        quote,
    );
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commonmark_preserves_command_blocks_and_link_formatting() {
        let blocks = parse_markdown(
            "# Networks\n\nUse **Layer 2** and [routes](routes.md).\n\n1. First\n2. Second\n\n```sh\nping -c 3 10.0.10.10\nip -br a\n```\n",
        );
        assert!(
            matches!(&blocks[0], MarkdownBlock::Heading { anchor, .. } if anchor == "networks")
        );
        assert!(
            blocks
                .iter()
                .flat_map(|b| b.spans())
                .any(|s| s.bold && s.text == "Layer 2")
        );
        assert!(
            blocks
                .iter()
                .flat_map(|b| b.spans())
                .any(|s| s.link.as_deref() == Some("routes.md"))
        );
        assert!(blocks.iter().any(
            |b| matches!(b, MarkdownBlock::Code(code) if code == "ping -c 3 10.0.10.10\nip -br a\n")
        ));
        let list: Vec<_> = blocks
            .iter()
            .filter_map(|b| {
                if let MarkdownBlock::Text {
                    marker: Some(m), ..
                } = b
                {
                    Some(m.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(list, ["1.", "2."]);
    }
}

#[cfg(test)]
mod nested_list_tests {
    use super::*;
    #[test]
    fn tight_nested_lists_keep_parent_and_child_as_separate_items() {
        let blocks = parse_markdown("- Parent\n  - Child\n- Next\n");
        let items: Vec<_> = blocks
            .iter()
            .filter_map(|b| match b {
                MarkdownBlock::Text {
                    marker,
                    depth,
                    spans,
                    ..
                } => Some((
                    marker.as_deref(),
                    *depth,
                    spans.iter().map(|s| s.text.as_str()).collect::<String>(),
                )),
                _ => None,
            })
            .collect();
        assert_eq!(
            items,
            [
                (Some("•"), 1, "Parent".into()),
                (Some("•"), 2, "Child".into()),
                (Some("•"), 1, "Next".into())
            ]
        );
    }
}
