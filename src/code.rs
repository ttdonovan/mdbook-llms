//! Code blocks: lines mdBook hides in HTML are kept, without their marker.

use crate::{apply_edits, markdown_options};
use mdbook_markdown::pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use std::collections::HashMap;
use std::ops::Range;

/// How mdBook hides lines in one code block.
enum Hidden {
    Rust,
    Prefix(String),
}

/// Keeps the lines mdBook hides in HTML but drops their marker, so code stays
/// complete and valid. Follows mdBook's rules: `# ` in Rust, else the fence's
/// `hidelines=` attribute or `output.html.code.hidelines`.
pub fn reveal_hidden_lines(body: &str, hidelines: &HashMap<String, String>) -> String {
    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    let mut mode = None;
    for (event, range) in Parser::new_ext(body, markdown_options()).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                mode = hidden_mode(&info, hidelines);
            }
            Event::End(TagEnd::CodeBlock) => mode = None,
            Event::Text(_) => {
                let (Some(mode), Some(text)) = (&mode, body.get(range.clone())) else {
                    continue;
                };
                let revealed: String = text
                    .split_inclusive('\n')
                    .map(|line| reveal_line(line, mode))
                    .collect();
                if revealed != text {
                    edits.push((range, revealed));
                }
            }
            _ => {}
        }
    }
    apply_edits(body, edits)
}

/// mdBook treats the first word of the fence info as the language.
fn hidden_mode(info: &str, hidelines: &HashMap<String, String>) -> Option<Hidden> {
    let mut parts = info.split([' ', '\t', ',']).filter(|part| !part.is_empty());
    let language = parts.next()?;
    if language == "rust" {
        return Some(Hidden::Rust);
    }
    parts
        .find_map(|part| part.strip_prefix("hidelines="))
        .or_else(|| hidelines.get(language).map(String::as_str))
        .filter(|prefix| !prefix.is_empty())
        .map(|prefix| Hidden::Prefix(prefix.to_string()))
}

fn reveal_line(line: &str, mode: &Hidden) -> String {
    let content = line.trim_end_matches(['\r', '\n']);
    let ending = line.get(content.len()..).unwrap_or_default();
    let rest = content.trim_start();
    let indent = content
        .get(..content.len().saturating_sub(rest.len()))
        .unwrap_or_default();
    let revealed = match mode {
        Hidden::Rust => rest
            .strip_prefix('#')
            .and_then(|after| match after.chars().next() {
                // `##` escapes a literal `#`.
                Some('#') => Some(after),
                None => Some(""),
                Some(' ') => after.get(1..),
                Some(_) => None,
            }),
        Hidden::Prefix(prefix) => rest.strip_prefix(prefix.as_str()),
    };
    revealed.map_or_else(
        || line.to_string(),
        |code| format!("{indent}{code}{ending}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_lines_keep_their_code_and_lose_the_marker() {
        let none = HashMap::new();
        let rust = "```rust,ignore\n# use std::fs;\n#\n## literal\n#[derive(Debug)]\n    # indented\nlet x = 1;\n```\n";
        assert_eq!(
            reveal_hidden_lines(rust, &none),
            "```rust,ignore\nuse std::fs;\n\n# literal\n#[derive(Debug)]\n    indented\nlet x = 1;\n```\n"
        );

        let python = "```python\n~import os\nprint(1)\n```\n";
        let config = HashMap::from([("python".to_string(), "~".to_string())]);
        assert_eq!(
            reveal_hidden_lines(python, &config),
            "```python\nimport os\nprint(1)\n```\n"
        );
        assert_eq!(reveal_hidden_lines(python, &none), python);

        let attribute = "```python hidelines=!!!\n!!!import os\n~kept\n```\n";
        assert_eq!(
            reveal_hidden_lines(attribute, &config),
            "```python hidelines=!!!\nimport os\n~kept\n```\n"
        );
    }

    #[test]
    fn hidden_line_markers_outside_fences_are_left_alone() {
        let none = HashMap::new();
        let body = "# Title\n\n    # indented code\n\n- item\n\n  ```rust\n  # hidden\n  ```\n";
        assert_eq!(
            reveal_hidden_lines(body, &none),
            "# Title\n\n    # indented code\n\n- item\n\n  ```rust\n  hidden\n  ```\n"
        );
    }
}
