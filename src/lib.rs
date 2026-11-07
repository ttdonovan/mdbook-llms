//! An mdBook 0.5 renderer for the llms.txt convention.

mod code;
mod frontmatter;
mod html;
mod links;
mod render;

pub use frontmatter::split_frontmatter;

/// The name of the full text, which an `optional` entry may list.
const FULL: &str = "llms-full.txt";

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

#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool is a user-facing TOML option"
)]
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
    /// End each `llms.txt` entry with its file's size in tokens.
    pub sizes: bool,
    /// A file under `src/` whose text follows the header of `llms.txt` and `llms-full.txt`.
    pub preamble: Option<PathBuf>,
    /// Files listed under `## Optional` in `llms.txt`.
    pub optional: Vec<Optional>,
}

/// A file that is not a chapter, listed under `## Optional`: copied from
/// `src/`, or `llms-full.txt`, which this renderer writes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Optional {
    /// Its path under `src/` and in the output.
    pub path: PathBuf,
    /// The entry's link text. Defaults to the file name.
    pub title: Option<String>,
    /// Text after the link, as a chapter's `description` gives.
    pub description: Option<String>,
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
            sizes: false,
            preamble: None,
            optional: Vec::new(),
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

/// Renders the llms output for the book in `ctx`.
///
/// # Errors
///
/// Fails when the config can't be read, merged mode finds no HTML output, or
/// a file can't be written.
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

    let preamble = match &config.preamble {
        Some(path) => Some(read_source(ctx, path)?),
        None => None,
    };
    let texts: Vec<String> = pages
        .iter()
        .map(|page| {
            let body =
                links::rewrite_links(page.body, page.path, &links, links::LinkStyle::Relative);
            let body = code::reveal_hidden_lines(&body, &hidelines);
            format!("{}\n", body.trim_end())
        })
        .collect();
    let full = config.full.then(|| {
        render::full(
            ctx,
            &pages,
            &base,
            extension,
            &links,
            &hidelines,
            preamble.as_deref(),
        )
    });
    let optional = optional_files(ctx, &config, full.as_deref())?;

    // Every generated file, kept to check its links once all are written.
    let mut written: Vec<(PathBuf, String)> = Vec::new();
    if config.index {
        let sizes: Option<HashMap<&Path, usize>> = config.sizes.then(|| {
            pages
                .iter()
                .zip(&texts)
                .map(|(page, text)| (page.path, text.len()))
                .collect()
        });
        let entries = optional_entries(&config, &optional, &base);
        let index = render::index(
            ctx,
            &pages,
            &render::IndexParts {
                base: &base,
                extension,
                preamble: preamble.as_deref(),
                sizes: sizes.as_ref(),
                optional: &entries,
            },
        );
        fs::write(dest.join("llms.txt"), &index)?;
        written.push((PathBuf::from("llms.txt"), index));
    }
    if let Some(full) = &full {
        // Not checked: its links come from the pages, which are.
        fs::write(dest.join(FULL), full)?;
    }
    for (entry, contents) in config.optional.iter().zip(&optional) {
        if entry.path != Path::new(FULL) {
            let path = dest.join(&entry.path);
            fs::create_dir_all(
                path.parent()
                    .context("optional file has no parent directory")?,
            )?;
            fs::write(path, contents)?;
        }
    }
    if config.pages {
        for (page, text) in pages.iter().zip(texts) {
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

/// The `## Optional` entries, each with its contents' size when entries show sizes.
fn optional_entries<'a>(
    config: &'a Config,
    contents: &[Vec<u8>],
    base: &str,
) -> Vec<render::OptionalEntry<'a>> {
    config
        .optional
        .iter()
        .zip(contents)
        .map(|(entry, contents)| render::OptionalEntry {
            title: entry.title.clone().unwrap_or_else(|| {
                entry
                    .path
                    .file_name()
                    .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
            }),
            url: format!("{base}{}", web_path(&entry.path)),
            description: entry.description.as_deref(),
            size: config.sizes.then_some(contents.len()),
        })
        .collect()
}

/// A file under the book's `src/`, by its path there.
fn read_source(ctx: &RenderContext, path: &Path) -> Result<String> {
    let full = ctx.source_dir().join(path);
    fs::read_to_string(&full).with_context(|| format!("can't read {}", full.display()))
}

/// Each `optional` entry's contents, in order: `llms-full.txt` as written,
/// any other file read from `src/`.
fn optional_files(
    ctx: &RenderContext,
    config: &Config,
    full: Option<&str>,
) -> Result<Vec<Vec<u8>>> {
    config
        .optional
        .iter()
        .map(|entry| match full {
            Some(full) if entry.path == Path::new(FULL) => Ok(full.as_bytes().to_vec()),
            _ => {
                let path = ctx.source_dir().join(&entry.path);
                fs::read(&path)
                    .with_context(|| format!("optional file {} not found", path.display()))
            }
        })
        .collect()
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
