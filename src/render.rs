//! `llms.txt` and `llms-full.txt`.

use crate::code::reveal_hidden_lines;
use crate::links::{LinkStyle, rewrite_links};
use crate::{Page, page_url};
use mdbook_renderer::RenderContext;
use mdbook_renderer::book::{BookItem, Chapter};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// What the index needs beyond the book.
pub struct IndexParts<'a> {
    pub base: &'a str,
    pub extension: &'a str,
    pub preamble: Option<&'a str>,
    /// Each page's size in bytes as written, when entries show sizes.
    pub sizes: Option<&'a HashMap<&'a Path, usize>>,
    pub optional: &'a [OptionalEntry<'a>],
}

/// A file listed under `## Optional`.
pub struct OptionalEntry<'a> {
    pub title: String,
    pub url: String,
    pub description: Option<&'a str>,
    pub size: Option<usize>,
}

/// The index: one line per chapter, grouped by `SUMMARY.md` parts and nested
/// as in the book, then any optional files.
pub fn index(ctx: &RenderContext, pages: &[Page], parts: &IndexParts) -> String {
    let index = Index {
        pages: pages.iter().map(|page| (page.path, page)).collect(),
        base: parts.base,
        extension: parts.extension,
        sizes: parts.sizes,
    };
    let mut out = header(ctx, parts.preamble);
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
    if !parts.optional.is_empty() {
        out.push_str("## Optional\n\n");
        for entry in parts.optional {
            out.push_str(&entry_line(
                "",
                &entry.title,
                &entry.url,
                entry.description,
                entry.size,
            ));
        }
    }
    format!("{}\n", out.trim_end())
}

/// One entry: its link, its description, and its size when known.
fn entry_line(
    indent: &str,
    title: &str,
    url: &str,
    description: Option<&str>,
    size: Option<usize>,
) -> String {
    let description = description.map(|d| format!(": {d}")).unwrap_or_default();
    let size = size
        .map(|bytes| format!(" ({})", tokens(bytes)))
        .unwrap_or_default();
    format!("{indent}- [{title}]({url}){description}{size}\n")
}

/// A size in tokens, estimated as a quarter of its bytes: to the hundred
/// below a thousand, else to a tenth of a thousand, both rounded up.
pub fn tokens(bytes: usize) -> String {
    let estimate = bytes.div_ceil(4);
    if estimate < 1000 {
        format!("about {}00 tokens", estimate.div_ceil(100).max(1))
    } else {
        let tenths = estimate.div_ceil(100);
        format!("about {}.{}k tokens", tenths / 10, tenths % 10)
    }
}

struct Index<'a> {
    pages: HashMap<&'a Path, &'a Page<'a>>,
    base: &'a str,
    extension: &'a str,
    sizes: Option<&'a HashMap<&'a Path, usize>>,
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
            let size = self.sizes.and_then(|sizes| sizes.get(page.path).copied());
            out.push_str(&entry_line(
                &"  ".repeat(depth),
                &chapter.name,
                &url,
                page.description.as_deref(),
                size,
            ));
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
    preamble: Option<&str>,
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
    format!(
        "{}---\n\n{}\n",
        header(ctx, preamble),
        pages.join("\n\n---\n\n")
    )
}

fn header(ctx: &RenderContext, preamble: Option<&str>) -> String {
    let title = ctx.config.book.title.as_deref().unwrap_or("Untitled");
    let mut out = format!("# {title}\n\n");
    if let Some(description) = &ctx.config.book.description {
        let _ = write!(out, "> {description}\n\n");
    }
    if let Some(preamble) = preamble.map(str::trim).filter(|p| !p.is_empty()) {
        let _ = write!(out, "{preamble}\n\n");
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
    fn sizes_round_up_to_hundreds_then_tenths_of_a_thousand() {
        assert_eq!(tokens(0), "about 100 tokens");
        assert_eq!(tokens(1_600), "about 400 tokens");
        assert_eq!(tokens(3_996), "about 1000 tokens");
        assert_eq!(tokens(4_000), "about 1.0k tokens");
        assert_eq!(tokens(5_604), "about 1.5k tokens");
    }

    #[test]
    fn an_entry_shows_its_description_then_its_size() {
        assert_eq!(
            entry_line(
                "  ",
                "Mining",
                "fleet/mining.md",
                Some("What a fleet mines"),
                Some(2_000)
            ),
            "  - [Mining](fleet/mining.md): What a fleet mines (about 500 tokens)\n"
        );
        assert_eq!(
            entry_line("", "data", "data.json", None, None),
            "- [data](data.json)\n"
        );
    }

    #[test]
    fn strips_only_a_matching_title() {
        assert_eq!(without_leading_title("# Intro\n\nBody", "Intro"), "Body");
        assert_eq!(
            without_leading_title("# Other\n\nBody", "Intro"),
            "# Other\n\nBody"
        );
    }
}
