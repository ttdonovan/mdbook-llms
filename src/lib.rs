use anyhow::{Context, Result, bail};
use mdbook_markdown::pulldown_cmark::{Event, Options, Parser, Tag};
use mdbook_renderer::RenderContext;
use mdbook_renderer::book::{BookItem, Chapter};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            index: true,
            full: true,
            pages: true,
            site_url: None,
            merge_into_html: true,
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

pub fn run(ctx: &RenderContext) -> Result<()> {
    let config: Config = ctx.config.get("output.llms")?.unwrap_or_default();
    let html_runs = ctx.config.contains_key("output.html");
    let base = base_url(ctx, &config)?;

    let dest = if html_runs && config.merge_into_html {
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

    let links = link_targets(ctx);
    let extension = if config.pages { "md" } else { "html" };

    if config.index {
        let index = render_index(ctx, &base, extension);
        fs::write(dest.join("llms.txt"), index)?;
    }
    if config.full {
        let full = render_full(ctx, &base, extension, &links);
        fs::write(dest.join("llms-full.txt"), full)?;
    }
    if config.pages {
        for chapter in ctx.book.chapters() {
            let Some(page) = Page::new(chapter) else {
                continue;
            };
            let path = dest.join(page.path);
            fs::create_dir_all(path.parent().context("page has no parent directory")?)?;
            let body = rewrite_links(page.body, page.path, &links, LinkStyle::Relative);
            fs::write(path, format!("{}\n", body.trim_end()))?;
        }
    }
    Ok(())
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

/// Splits YAML frontmatter from a chapter, returning its description and the
/// remaining body. mdBook leaves frontmatter in chapter content. Only a block
/// of YAML-shaped lines counts, so a chapter opening with a `---` rule survives.
pub fn split_frontmatter(content: &str) -> (Option<String>, &str) {
    let mut lines = content.split_inclusive('\n');
    if lines.next().map(str::trim_end) != Some("---") {
        return (None, content);
    }

    let mut offset = content.split_inclusive('\n').next().map_or(0, str::len);
    let mut description = None;
    for line in lines {
        offset += line.len();
        let text = line.trim_end();
        if text == "---" {
            return (
                description,
                content[offset..].trim_start_matches(['\r', '\n']),
            );
        }
        if !is_yaml_line(text) {
            return (None, content);
        }
        if let Some(value) = text.strip_prefix("description:") {
            let value = value.trim().trim_matches(['"', '\'']);
            description = Some(value.to_string()).filter(|v| !v.is_empty());
        }
    }
    (None, content)
}

fn is_yaml_line(line: &str) -> bool {
    line.is_empty()
        || line.starts_with([' ', '\t', '#'])
        || line.starts_with("- ")
        || line.split_once(':').is_some_and(|(key, _)| {
            !key.is_empty() && key.chars().all(|c| c.is_alphanumeric() || "-_".contains(c))
        })
}

fn render_index(ctx: &RenderContext, base: &str, extension: &str) -> String {
    let mut out = header(ctx);
    let mut heading_open = false;
    let mut items = String::new();

    for item in &ctx.book.items {
        match item {
            BookItem::PartTitle(title) => {
                flush_section(&mut out, &mut items, &mut heading_open);
                out.push_str(&format!("## {title}\n\n"));
                heading_open = true;
            }
            BookItem::Chapter(chapter) => {
                list_chapter(chapter, 0, base, extension, &mut items);
            }
            _ => {}
        }
    }
    flush_section(&mut out, &mut items, &mut heading_open);
    format!("{}\n", out.trim_end())
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

fn list_chapter(chapter: &Chapter, depth: usize, base: &str, extension: &str, out: &mut String) {
    // A draft has no page, so its children move up a level rather than vanish.
    let child_depth = match Page::new(chapter) {
        Some(page) => {
            let url = page_url(base, page.path, extension);
            let description = page
                .description
                .map(|d| format!(": {d}"))
                .unwrap_or_default();
            let indent = "  ".repeat(depth);
            out.push_str(&format!(
                "{indent}- [{}]({url}){description}\n",
                chapter.name
            ));
            depth + 1
        }
        None => depth,
    };
    for item in &chapter.sub_items {
        if let BookItem::Chapter(sub) = item {
            list_chapter(sub, child_depth, base, extension, out);
        }
    }
}

fn render_full(
    ctx: &RenderContext,
    base: &str,
    extension: &str,
    links: &HashMap<PathBuf, PathBuf>,
) -> String {
    let pages: Vec<String> = ctx
        .book
        .chapters()
        .filter_map(Page::new)
        .map(|page| {
            let body = without_leading_title(page.body, &page.chapter.name);
            let style = LinkStyle::Absolute { base, extension };
            let body = rewrite_links(body, page.path, links, style);
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
        out.push_str(&format!("> {description}\n\n"));
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

/// Every way a chapter can be addressed, mapped to its published path. Authors
/// link to `README.md`, but mdBook publishes that chapter as `index.md`.
fn link_targets(ctx: &RenderContext) -> HashMap<PathBuf, PathBuf> {
    let mut targets = HashMap::new();
    for chapter in ctx.book.chapters() {
        let Some(path) = &chapter.path else { continue };
        targets.insert(path.clone(), path.clone());
        if let Some(source) = &chapter.source_path {
            targets.insert(source.clone(), path.clone());
        }
    }
    targets
}

#[derive(Clone, Copy)]
enum LinkStyle<'a> {
    /// Stays relative to the chapter's own file. Used for per-page Markdown.
    Relative,
    /// Must survive being read outside the chapter's directory.
    Absolute { base: &'a str, extension: &'a str },
}

/// Points links between chapters at their published locations. Uses a real
/// Markdown parser, so anything that looks like a link inside code is left be.
fn rewrite_links(
    body: &str,
    chapter: &Path,
    links: &HashMap<PathBuf, PathBuf>,
    style: LinkStyle,
) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(body, options);

    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    for (_, definition) in parser.reference_definitions().iter() {
        if let Some(new) = resolve(&definition.dest, chapter, links, style)
            && let Some(at) = find_in(body, definition.span.clone(), &definition.dest)
        {
            edits.push((at, new));
        }
    }
    for (event, span) in parser.into_offset_iter() {
        let Event::Start(Tag::Link { dest_url, .. }) = event else {
            continue;
        };
        if let Some(new) = resolve(&dest_url, chapter, links, style) {
            // Search backwards: the link text could contain the same string.
            let slice = &body[span.clone()];
            if let Some(offset) = slice.rfind(dest_url.as_ref()) {
                let start = span.start + offset;
                edits.push((start..start + dest_url.len(), new));
            }
        }
    }

    edits.sort_by_key(|(range, _)| range.start);
    edits.dedup_by_key(|(range, _)| range.start);
    let mut out = body.to_string();
    for (range, new) in edits.into_iter().rev() {
        out.replace_range(range, &new);
    }
    out
}

fn find_in(body: &str, span: Range<usize>, needle: &str) -> Option<Range<usize>> {
    let start = span.start + body[span].find(needle)?;
    Some(start..start + needle.len())
}

fn resolve(
    dest: &str,
    chapter: &Path,
    links: &HashMap<PathBuf, PathBuf>,
    style: LinkStyle,
) -> Option<String> {
    let (path, fragment) = match dest.split_once('#') {
        Some((path, fragment)) => (path, format!("#{fragment}")),
        None => (dest, String::new()),
    };
    if !path.ends_with(".md") || path.starts_with('/') || path.contains("://") {
        return None;
    }
    let dir = chapter.parent().unwrap_or(Path::new(""));
    let target = links.get(&normalize(&dir.join(path)))?;
    match style {
        LinkStyle::Absolute { base, extension } => {
            Some(format!("{}{fragment}", page_url(base, target, extension)))
        }
        LinkStyle::Relative => {
            // Only the filename can differ: mdBook keeps README in its directory.
            let name = target.file_name()?.to_string_lossy();
            let (prefix, _) = path.rsplit_once('/').unwrap_or(("", path));
            let joined = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{prefix}/{name}")
            };
            (joined != path).then(|| format!("{joined}{fragment}"))
        }
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links() -> HashMap<PathBuf, PathBuf> {
        HashMap::from([
            (
                PathBuf::from("guide/README.md"),
                PathBuf::from("guide/index.md"),
            ),
            (
                PathBuf::from("guide/intro.md"),
                PathBuf::from("guide/intro.md"),
            ),
            (PathBuf::from("api.md"), PathBuf::from("api.md")),
        ])
    }

    const ABSOLUTE: LinkStyle = LinkStyle::Absolute {
        base: "https://x.dev/",
        extension: "md",
    };

    fn absolute(body: &str) -> String {
        rewrite_links(body, Path::new("guide/intro.md"), &links(), ABSOLUTE)
    }

    #[test]
    fn resolves_relative_links_against_the_chapter() {
        assert_eq!(absolute("[a](../api.md#x)"), "[a](https://x.dev/api.md#x)");
        assert_eq!(
            absolute("[a](README.md)"),
            "[a](https://x.dev/guide/index.md)"
        );
    }

    #[test]
    fn rewrites_reference_definitions() {
        assert_eq!(
            absolute("[a][r]\n\n[r]: ../api.md\n"),
            "[a][r]\n\n[r]: https://x.dev/api.md\n"
        );
    }

    #[test]
    fn leaves_code_external_and_unknown_links_alone() {
        for body in [
            "`[a](../api.md)`",
            "```\n[a](../api.md)\n```",
            "[a](https://y.dev/api.md)",
            "[a](missing.md)",
            "[a](#local)",
        ] {
            assert_eq!(absolute(body), body);
        }
    }

    #[test]
    fn relative_style_only_renames_readme() {
        let relative =
            |body| rewrite_links(body, Path::new("api.md"), &links(), LinkStyle::Relative);
        assert_eq!(relative("[a](guide/README.md#x)"), "[a](guide/index.md#x)");
        assert_eq!(relative("[a](guide/intro.md)"), "[a](guide/intro.md)");
    }

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
    fn an_unclosed_block_is_not_frontmatter() {
        let content = "---\ntitle: Hi\n\nBody\n";
        assert_eq!(split_frontmatter(content), (None, content));
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
