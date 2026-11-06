//! `llms.txt` and `llms-full.txt`.

use crate::code::reveal_hidden_lines;
use crate::links::{LinkStyle, rewrite_links};
use crate::{Page, page_url};
use mdbook_renderer::RenderContext;
use mdbook_renderer::book::{BookItem, Chapter};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The index: one line per chapter, grouped by `SUMMARY.md` parts and nested
/// as in the book.
pub fn index(ctx: &RenderContext, pages: &[Page], base: &str, extension: &str) -> String {
    let index = Index {
        pages: pages.iter().map(|page| (page.path, page)).collect(),
        base,
        extension,
    };
    let mut out = header(ctx);
    let mut heading_open = false;
    let mut items = String::new();

    for item in &ctx.book.items {
        match item {
            BookItem::PartTitle(title) => {
                flush_section(&mut out, &mut items, &mut heading_open);
                let _ = write!(out, "## {title}\n\n");
                heading_open = true;
            }
            BookItem::Chapter(chapter) => index.list(chapter, 0, &mut items),
            BookItem::Separator => {}
        }
    }
    flush_section(&mut out, &mut items, &mut heading_open);
    format!("{}\n", out.trim_end())
}

struct Index<'a> {
    pages: HashMap<&'a Path, &'a Page<'a>>,
    base: &'a str,
    extension: &'a str,
}

impl Index<'_> {
    fn list(&self, chapter: &Chapter, depth: usize, out: &mut String) {
        let page = chapter
            .path
            .as_deref()
            .and_then(|path| self.pages.get(path));
        // A draft has no page, so its children move up a level rather than vanish.
        let child_depth = page.map_or(depth, |page| {
            let url = page_url(self.base, page.path, self.extension);
            let description = page
                .description
                .as_ref()
                .map(|d| format!(": {d}"))
                .unwrap_or_default();
            let indent = "  ".repeat(depth);
            let _ = writeln!(out, "{indent}- [{}]({url}){description}", chapter.name);
            depth.saturating_add(1)
        });
        for item in &chapter.sub_items {
            if let BookItem::Chapter(sub) = item {
                self.list(sub, child_depth, out);
            }
        }
    }
}

fn flush_section(out: &mut String, items: &mut String, heading_open: &mut bool) {
    if items.is_empty() {
        return;
    }
    if !*heading_open {
        out.push_str("## Contents\n\n");
    }
    out.push_str(items);
    out.push('\n');
    items.clear();
    *heading_open = false;
}

/// Every chapter in reading order, each with a `Source:` line.
pub fn full(
    ctx: &RenderContext,
    pages: &[Page],
    base: &str,
    extension: &str,
    links: &HashMap<PathBuf, PathBuf>,
    hidelines: &HashMap<String, String>,
) -> String {
    let style = LinkStyle::Absolute { base, extension };
    let pages: Vec<String> = pages
        .iter()
        .map(|page| {
            let body = without_leading_title(page.body, &page.chapter.name);
            let body = rewrite_links(body, page.path, links, style);
            let body = reveal_hidden_lines(&body, hidelines);
            let source = page_url(base, page.path, extension);
            format!(
                "# {}\n\nSource: {source}\n\n{}",
                page.chapter.name,
                body.trim()
            )
        })
        .collect();
    format!("{}---\n\n{}\n", header(ctx), pages.join("\n\n---\n\n"))
}

fn header(ctx: &RenderContext) -> String {
    let title = ctx.config.book.title.as_deref().unwrap_or("Untitled");
    let mut out = format!("# {title}\n\n");
    if let Some(description) = &ctx.config.book.description {
        let _ = write!(out, "> {description}\n\n");
    }
    out
}

/// The entry prints the title itself, so a leading `# Title` would repeat it.
fn without_leading_title<'a>(body: &'a str, title: &str) -> &'a str {
    let trimmed = body.trim_start();
    let (first, rest) = trimmed.split_once('\n').unwrap_or((trimmed, ""));
    match first.strip_prefix("# ") {
        Some(heading) if heading.trim() == title.trim() => rest.trim_start(),
        _ => body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_only_a_matching_title() {
        assert_eq!(without_leading_title("# Intro\n\nBody", "Intro"), "Body");
        assert_eq!(
            without_leading_title("# Other\n\nBody", "Intro"),
            "# Other\n\nBody"
        );
    }
}
