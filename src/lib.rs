//! An mdBook 0.5 renderer for the llms.txt convention.

mod code;
mod frontmatter;
mod html;
mod links;
mod render;

pub use frontmatter::split_frontmatter;

use anyhow::{Context, Result, bail};
use mdbook_markdown::pulldown_cmark::Options;
use mdbook_renderer::RenderContext;
use mdbook_renderer::book::Chapter;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

const HTML_OUTPUT: &str = "html";

// ANCHOR: config
#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Config {
    /// Write `llms.txt`.
    pub index: bool,
    /// Write `llms-full.txt`.
    pub full: bool,
    /// Write one `.md` file per chapter. When off, `llms.txt` links to `.html`.
    pub pages: bool,
    /// Prefix for links. Falls back to `output.html.site-url`.
    pub site_url: Option<String>,
    /// When the HTML renderer also runs, publish into its directory.
    pub merge_into_html: bool,
    /// When merged, add llms.txt v2 `alternate` and `describedby` links to HTML pages.
    pub discovery_links: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            index: true,
            full: true,
            pages: true,
            site_url: None,
            merge_into_html: true,
            discovery_links: true,
        }
    }
}
// ANCHOR_END: config

/// A chapter that has a file behind it, cleaned up for plain-text readers.
struct Page<'a> {
    chapter: &'a Chapter,
    path: &'a Path,
    description: Option<String>,
    body: &'a str,
}

impl<'a> Page<'a> {
    fn new(chapter: &'a Chapter) -> Option<Self> {
        let path = chapter.path.as_deref()?;
        let (description, body) = split_frontmatter(&chapter.content);
        Some(Self {
            chapter,
            path,
            description,
            body,
        })
    }
}

pub fn run(ctx: &RenderContext) -> Result<()> {
    let config: Config = ctx.config.get("output.llms")?.unwrap_or_default();
    let merged = ctx.config.contains_key("output.html") && config.merge_into_html;
    let base = base_url(ctx, &config)?;
    let dest = output_dir(ctx, merged)?;

    let pages: Vec<Page> = ctx.book.chapters().filter_map(Page::new).collect();
    let links = links::link_targets(&pages);
    let hidelines: HashMap<String, String> = ctx
        .config
        .get("output.html.code.hidelines")?
        .unwrap_or_default();
    let extension = if config.pages { "md" } else { "html" };

    // Every generated file, kept to check its links once all are written.
    let mut written: Vec<(PathBuf, String)> = Vec::new();
    if config.index {
        let index = render::index(ctx, &pages, &base, extension);
        fs::write(dest.join("llms.txt"), &index)?;
        written.push((PathBuf::from("llms.txt"), index));
    }
    if config.full {
        let full = render::full(ctx, &pages, &base, extension, &links, &hidelines);
        // Not checked: its links come from the pages, which are.
        fs::write(dest.join("llms-full.txt"), full)?;
    }
    if config.pages {
        for page in &pages {
            let body =
                links::rewrite_links(page.body, page.path, &links, links::LinkStyle::Relative);
            let body = code::reveal_hidden_lines(&body, &hidelines);
            let text = format!("{}\n", body.trim_end());
            let path = dest.join(page.path);
            fs::create_dir_all(path.parent().context("page has no parent directory")?)?;
            fs::write(path, &text)?;
            written.push((page.path.to_path_buf(), text));
        }
    }
    if merged && config.discovery_links {
        html::add_discovery_links(&dest, &pages, &base, &config)?;
    }
    warn_broken_links(ctx, &dest, merged, &base, &written);
    Ok(())
}

/// The HTML directory when merged, else this renderer's own, emptied first.
fn output_dir(ctx: &RenderContext, merged: bool) -> Result<PathBuf> {
    let dest = if merged {
        // mdBook gives each backend a sibling directory when there are several.
        let html = ctx
            .destination
            .parent()
            .context("renderer destination has no parent")?
            .join(HTML_OUTPUT);
        // mdBook runs outputs alphabetically, so HTML has already finished.
        if !html.join("index.html").exists() {
            bail!(
                "merge-into-html found no HTML output in {}; set \
                 merge-into-html = false to write to {} instead",
                html.display(),
                ctx.destination.display()
            );
        }
        html
    } else {
        clean_stale_output(ctx)?;
        ctx.destination.clone()
    };
    fs::create_dir_all(&dest)?;
    Ok(dest)
}

/// Standalone builds own their directory, the same way the HTML renderer owns
/// its own. Refuses anything outside the build directory.
fn clean_stale_output(ctx: &RenderContext) -> Result<()> {
    let build_dir = ctx.root.join(&ctx.config.build.build_dir);
    if !ctx.destination.starts_with(&build_dir) || !ctx.destination.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&ctx.destination)? {
        let path = entry?.path();
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn warn_broken_links(
    ctx: &RenderContext,
    dest: &Path,
    merged: bool,
    base: &str,
    written: &[(PathBuf, String)],
) {
    let src = ctx.root.join(&ctx.config.book.src);
    let exists = |target: &Path| {
        let extension = target.extension().and_then(|ext| ext.to_str());
        // Without the HTML output, .html targets can't be checked here.
        (!merged && extension == Some("html"))
            || dest.join(target).exists()
            // Assets stay in src/; a chapter must have been written.
            || (extension != Some("md") && src.join(target).exists())
    };
    for (file, link) in links::broken_links(written, base, exists) {
        eprintln!(
            "warning: mdbook-llms: broken link in {}: {link}",
            file.display()
        );
    }
}

fn base_url(ctx: &RenderContext, config: &Config) -> Result<String> {
    let site = match &config.site_url {
        Some(url) => Some(url.clone()),
        None => ctx.config.get::<String>("output.html.site-url")?,
    };
    Ok(site
        .map(|url| format!("{}/", url.trim_end_matches('/')))
        .unwrap_or_default())
}

fn page_url(base: &str, path: &Path, extension: &str) -> String {
    format!("{base}{}", web_path(&path.with_extension(extension)))
}

fn web_path(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
}

/// Replaces each byte range of `body`. Edits that overlap an earlier one, or
/// fall outside `body`, are dropped rather than risk corrupting the text.
fn apply_edits(body: &str, mut edits: Vec<(Range<usize>, String)>) -> String {
    edits.sort_by_key(|(range, _)| range.start);
    let mut out = String::with_capacity(body.len());
    let mut last = 0;
    for (range, new) in edits {
        let Some(kept) = body.get(last..range.start) else {
            continue;
        };
        if body.get(range.clone()).is_none() {
            continue;
        }
        out.push_str(kept);
        out.push_str(&new);
        last = range.end;
    }
    out.push_str(body.get(last..).unwrap_or_default());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_apply_in_order_and_skip_overlaps() {
        let edits = vec![
            (6..11, "there".to_string()),
            (0..5, "Hi".to_string()),
            (8..9, "x".to_string()),
            (20..30, "out of range".to_string()),
        ];
        assert_eq!(apply_edits("hello world!", edits), "Hi there!");
    }
}
