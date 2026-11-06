//! YAML frontmatter: detection, removal, and the `description` value.

/// Splits YAML frontmatter from a chapter, returning its description and body.
///
/// mdBook leaves frontmatter in chapter content. Only a block of YAML-shaped
/// lines counts, so a chapter opening with a `---` rule survives.
pub fn split_frontmatter(content: &str) -> (Option<String>, &str) {
    let mut lines = content.split_inclusive('\n');
    if lines.next().map(str::trim_end) != Some("---") {
        return (None, content);
    }

    let mut offset = content.split_inclusive('\n').next().map_or(0, str::len);
    let mut block = Vec::new();
    for line in lines {
        offset = offset.saturating_add(line.len());
        let text = line.trim_end();
        if text == "---" {
            let body = content.get(offset..).unwrap_or_default();
            return (description(&block), body.trim_start_matches(['\r', '\n']));
        }
        if !is_yaml_line(text) {
            return (None, content);
        }
        block.push(text);
    }
    (None, content)
}

/// The top-level `description` value, with its indented continuation lines.
fn description(block: &[&str]) -> Option<String> {
    let mut lines = block.iter();
    let head = lines.find_map(|line| line.strip_prefix("description:"))?;
    let rest: Vec<&str> = lines
        .take_while(|line| line.is_empty() || line.starts_with([' ', '\t']))
        .copied()
        .collect();
    let value = scalar(head.trim(), &rest)?;
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(value).filter(|v| !v.is_empty())
}

/// A YAML scalar flattened to one line: plain, quoted, folded or literal.
/// Returns `None` for an unclosed quote rather than guess.
fn scalar(head: &str, rest: &[&str]) -> Option<String> {
    let joined = std::iter::once(head)
        .chain(rest.iter().map(|line| line.trim()))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    match head.chars().next() {
        // Block scalar: the header line holds only indicators (and maybe a comment).
        Some('>' | '|') => Some(
            rest.iter()
                .map(|line| line.trim())
                .collect::<Vec<_>>()
                .join(" "),
        ),
        Some('\'') => single_quoted(&joined),
        Some('"') => double_quoted(&joined),
        _ => Some(
            joined
                .split_once(" #")
                .map_or(joined.as_str(), |(value, _)| value)
                .to_string(),
        ),
    }
}

fn single_quoted(text: &str) -> Option<String> {
    let mut chars = text.chars().skip(1).peekable();
    let mut out = String::new();
    while let Some(c) = chars.next() {
        if c != '\'' {
            out.push(c);
        } else if chars.next_if_eq(&'\'').is_some() {
            out.push('\'');
        } else {
            return Some(out);
        }
    }
    None
}

fn double_quoted(text: &str) -> Option<String> {
    let mut chars = text.chars().skip(1);
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' | 't' => out.push(' '),
                escaped @ ('"' | '\\' | '/') => out.push(escaped),
                other => {
                    out.push('\\');
                    out.push(other);
                }
            },
            _ => out.push(c),
        }
    }
    None
}

fn is_yaml_line(line: &str) -> bool {
    line.is_empty()
        || line.starts_with([' ', '\t', '#'])
        || line.starts_with("- ")
        || line.split_once(':').is_some_and(|(key, _)| {
            !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || "-_".contains(c))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_gives_a_description_and_is_removed() {
        let (description, body) =
            split_frontmatter("---\ndescription: \"Hi\"\norder: 1\n---\n\n# T\n");
        assert_eq!(description.as_deref(), Some("Hi"));
        assert_eq!(body, "# T\n");
    }

    #[test]
    fn a_leading_rule_is_not_frontmatter() {
        let content = "---\n\nSome prose here.\n\n---\n\nMore.\n";
        assert_eq!(split_frontmatter(content), (None, content));
    }

    #[test]
    fn frontmatter_can_hold_lists_and_crlf() {
        let (description, body) =
            split_frontmatter("---\ndescription: Hi\ntags:\n- a\n- b\n---\n# T\n");
        assert_eq!((description.as_deref(), body), (Some("Hi"), "# T\n"));
        let (description, body) = split_frontmatter("---\r\ndescription: Hi\r\n---\r\n\r\n# T\r\n");
        assert_eq!((description.as_deref(), body), (Some("Hi"), "# T\r\n"));
    }

    #[test]
    fn description_scalars_are_flattened_to_one_line() {
        let cases = [
            ("description: Install the CLI", Some("Install the CLI")),
            (
                "description: Install\n  the CLI\ntags:\n- a",
                Some("Install the CLI"),
            ),
            ("description: Install # todo", Some("Install")),
            (
                "description: >-\n  How to install\n  the CLI",
                Some("How to install the CLI"),
            ),
            (
                "description: >\n  Folded\n\n  paragraphs\n",
                Some("Folded paragraphs"),
            ),
            (
                "description: |\n  Line one\n  Line two\norder: 1",
                Some("Line one Line two"),
            ),
            (
                "description: 'It''s here: really'",
                Some("It's here: really"),
            ),
            ("description: 'Two\n  lines'", Some("Two lines")),
            (
                r#"description: "Say \"hi\"\nnow # kept""#,
                Some(r#"Say "hi" now # kept"#),
            ),
            ("description: >-", None),
            ("description: 'unclosed", None),
            ("description:", None),
        ];
        for (yaml, expected) in cases {
            let content = format!("---\n{yaml}\n---\n# T\n");
            let (description, body) = split_frontmatter(&content);
            assert_eq!(description.as_deref(), expected, "{yaml}");
            assert_eq!(body, "# T\n", "{yaml}");
        }
    }

    #[test]
    fn an_unclosed_block_is_not_frontmatter() {
        let content = "---\ntitle: Hi\n\nBody\n";
        assert_eq!(split_frontmatter(content), (None, content));
    }
}
