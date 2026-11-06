//! llms.txt v2 discovery links in the finished HTML pages.

use crate::{Config, Page, web_path};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Points each HTML page at its Markdown and at `llms.txt`, as llms.txt v2
/// recommends. Runs after the HTML renderer, so it edits finished pages.
pub(crate) fn add_discovery_links(
    html_dir: &Path,
    pages: &[Page],
    base: &str,
    config: &Config,
) -> Result<()> {
    let link_tags = |html: &Path, markdown: Option<&Path>| {
        let mut tags = String::new();
        if let Some(markdown) = markdown.filter(|_| config.pages) {
            let href = href(base, html, &web_path(markdown));
            tags.push_str(&format!(
                "<link rel=\"alternate\" type=\"text/markdown\" href=\"{}\">\n",
                escape_attr(&href)
            ));
        }
        if config.index {
            let href = href(base, html, "llms.txt");
            tags.push_str(&format!(
                "<link rel=\"describedby\" href=\"{}\">\n",
                escape_attr(&href)
            ));
        }
        tags
    };

    let mut targets: Vec<(PathBuf, Option<&Path>)> = pages
        .iter()
        .map(|page| (page.path.with_extension("html"), Some(page.path)))
        .collect();
    // mdBook copies the first chapter to index.html when no chapter is the index.
    if !pages.iter().any(|page| page.path == Path::new("index.md")) {
        targets.push((
            PathBuf::from("index.html"),
            pages.first().map(|page| page.path),
        ));
    }
    targets.push((PathBuf::from("print.html"), None));

    for (html, markdown) in targets {
        let file = html_dir.join(&html);
        let Ok(page) = fs::read_to_string(&file) else {
            continue;
        };
        if let Some(updated) = insert_before_head_end(&page, &link_tags(&html, markdown)) {
            fs::write(&file, updated)?;
        }
    }
    Ok(())
}

/// An absolute URL when a site URL is known, else relative to the HTML file.
fn href(base: &str, from: &Path, target: &str) -> String {
    if !base.is_empty() {
        return format!("{base}{target}");
    }
    let depth = from.components().count().saturating_sub(1);
    format!("{}{target}", "../".repeat(depth))
}

/// `None` when there is nothing to add, no `</head>`, or the tags are already there.
fn insert_before_head_end(page: &str, tags: &str) -> Option<String> {
    if tags.is_empty() || page.contains(tags) {
        return None;
    }
    let (head, rest) = page.split_once("</head>")?;
    Some(format!("{head}{tags}</head>{rest}"))
}

fn escape_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_hrefs_are_absolute_with_a_site_url_else_relative() {
        let page = Path::new("guide/intro.html");
        assert_eq!(href("/book/", page, "llms.txt"), "/book/llms.txt");
        assert_eq!(href("", page, "llms.txt"), "../llms.txt");
        assert_eq!(href("", page, "guide/intro.md"), "../guide/intro.md");
        assert_eq!(href("", Path::new("index.html"), "intro.md"), "intro.md");
    }

    #[test]
    fn discovery_tags_go_before_head_end_once() {
        let tags = "<link rel=\"describedby\" href=\"llms.txt\">\n";
        let page = "<html><head><title>T</title></head><body></body></html>";
        let updated = insert_before_head_end(page, tags).unwrap_or_default();
        assert_eq!(
            updated,
            "<html><head><title>T</title><link rel=\"describedby\" href=\"llms.txt\">\n</head><body></body></html>"
        );
        assert_eq!(insert_before_head_end(&updated, tags), None);
        assert_eq!(insert_before_head_end("<p>no head</p>", tags), None);
        assert_eq!(escape_attr("a\"b&c"), "a&quot;b&amp;c");
    }
}
