use anyhow::{Context, Result, bail};
use mdbook_markdown::pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
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

pub fn run(ctx: &RenderContext) -> Result<()> {
    let config: Config = ctx.config.get("output.llms")?.unwrap_or_default();
    let html_runs = ctx.config.contains_key("output.html");
    let base = base_url(ctx, &config)?;

    let merged = html_runs && config.merge_into_html;
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

    let links = link_targets(ctx);
    let hidelines: HashMap<String, String> = ctx
        .config
        .get("output.html.code.hidelines")?
        .unwrap_or_default();
    let extension = if config.pages { "md" } else { "html" };

    // Every generated file, kept to check its links once all are written.
    let mut written: Vec<(PathBuf, String)> = Vec::new();
    if config.index {
        let index = render_index(ctx, &base, extension);
        fs::write(dest.join("llms.txt"), &index)?;
        written.push((PathBuf::from("llms.txt"), index));
    }
    if config.full {
        let full = render_full(ctx, &base, extension, &links, &hidelines);
        // Not checked: its links come from the pages, which are.
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
            let body = reveal_hidden_lines(&body, &hidelines);
            let text = format!("{}\n", body.trim_end());
            fs::write(path, &text)?;
            written.push((page.path.to_path_buf(), text));
        }
    }
    if merged && config.discovery_links {
        add_discovery_links(ctx, &dest, &base, &config)?;
    }

    let src = ctx.root.join(&ctx.config.book.src);
    let exists = |target: &Path| {
        let extension = target.extension().and_then(|ext| ext.to_str());
        // Without the HTML output, .html targets can't be checked here.
        (!merged && extension == Some("html"))
            || dest.join(target).exists()
            // Assets stay in src/; a chapter must have been written.
            || (extension != Some("md") && src.join(target).exists())
    };
    for (file, link) in broken_links(&written, &base, exists) {
        eprintln!(
            "warning: mdbook-llms: broken link in {}: {link}",
            file.display()
        );
    }
    Ok(())
}

/// Relative links in the generated files whose target does not exist.
/// Anchors and external URLs are not checked.
fn broken_links(
    written: &[(PathBuf, String)],
    base: &str,
    exists: impl Fn(&Path) -> bool,
) -> Vec<(PathBuf, String)> {
    let mut broken = Vec::new();
    // One report per missing file, however many anchors point into it.
    let mut seen: Vec<(&PathBuf, PathBuf)> = Vec::new();
    for (file, text) in written {
        let from = file.parent().unwrap_or(Path::new(""));
        for event in Parser::new_ext(text, markdown_options()) {
            let (Event::Start(Tag::Link { dest_url, .. })
            | Event::Start(Tag::Image { dest_url, .. })) = event
            else {
                continue;
            };
            let Some(target) = link_target(&dest_url, from, base) else {
                continue;
            };
            if exists(&target) || seen.contains(&(file, target.clone())) {
                continue;
            }
            seen.push((file, target));
            broken.push((file.clone(), dest_url.to_string()));
        }
    }
    broken
}

/// The output-relative path a link points at, or `None` when it can't be
/// checked: external, fragment-only, or root-absolute outside the site URL.
fn link_target(dest: &str, from: &Path, base: &str) -> Option<PathBuf> {
    let path = dest
        .split(['#', '?'])
        .next()
        .filter(|path| !path.is_empty())?;
    if !base.is_empty()
        && let Some(rest) = path.strip_prefix(base)
    {
        return Some(normalize(Path::new(rest)));
    }
    let has_scheme = path
        .split_once(':')
        .is_some_and(|(scheme, _)| !scheme.contains('/'));
    if has_scheme || path.starts_with('/') {
        return None;
    }
    Some(normalize(&from.join(path)))
}

/// Points each HTML page at its Markdown and at `llms.txt`, as llms.txt v2
/// recommends. Runs after the HTML renderer, so it edits finished pages.
fn add_discovery_links(
    ctx: &RenderContext,
    html_dir: &Path,
    base: &str,
    config: &Config,
) -> Result<()> {
    let pages: Vec<&Path> = ctx
        .book
        .chapters()
        .filter_map(|chapter| chapter.path.as_deref())
        .collect();
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
        .map(|path| (path.with_extension("html"), Some(*path)))
        .collect();
    // mdBook copies the first chapter to index.html when no chapter is the index.
    if !pages.iter().any(|path| *path == Path::new("index.md")) {
        targets.push((PathBuf::from("index.html"), pages.first().copied()));
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
    hidelines: &HashMap<String, String>,
) -> String {
    let pages: Vec<String> = ctx
        .book
        .chapters()
        .filter_map(Page::new)
        .map(|page| {
            let body = without_leading_title(page.body, &page.chapter.name);
            let style = LinkStyle::Absolute { base, extension };
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
    let parser = Parser::new_ext(body, markdown_options());

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

fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
}

/// How mdBook hides lines in one code block.
enum Hidden {
    Rust,
    Prefix(String),
}

/// Keeps the lines mdBook hides in HTML but drops their marker, so code stays
/// complete and valid. Follows mdBook's rules: `# ` in Rust, else the fence's
/// `hidelines=` attribute or `output.html.code.hidelines`.
fn reveal_hidden_lines(body: &str, hidelines: &HashMap<String, String>) -> String {
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

    let mut out = String::with_capacity(body.len());
    let mut last = 0;
    for (range, new) in edits {
        out.push_str(body.get(last..range.start).unwrap_or_default());
        out.push_str(&new);
        last = range.end;
    }
    out.push_str(body.get(last..).unwrap_or_default());
    out
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
    match revealed {
        Some(code) => format!("{indent}{code}{ending}"),
        None => line.to_string(),
    }
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

    #[test]
    fn link_targets_resolve_from_the_file_or_the_site_url() {
        let guide = Path::new("guide");
        assert_eq!(
            link_target("intro.md#top", guide, ""),
            Some(PathBuf::from("guide/intro.md"))
        );
        assert_eq!(
            link_target("../img/a.png", guide, ""),
            Some(PathBuf::from("img/a.png"))
        );
        assert_eq!(
            link_target("/book/usage.md", Path::new(""), "/book/"),
            Some(PathBuf::from("usage.md"))
        );
        assert_eq!(
            link_target("/other/usage.md", Path::new(""), "/book/"),
            None
        );
        assert_eq!(link_target("/usage.md", Path::new(""), ""), None);
        assert_eq!(link_target("https://example.com/a.md", guide, ""), None);
        assert_eq!(link_target("mailto:a@example.com", guide, ""), None);
        assert_eq!(link_target("#section", guide, ""), None);
    }

    #[test]
    fn broken_links_are_reported_once_per_file() {
        let written = vec![
            (
                PathBuf::from("guide/intro.md"),
                "[ok](deep.md) [gone](missing.md) [again](missing.md#x) ![img](../a.png) [ref][r]\n\n[r]: ../nope.md\n".to_string(),
            ),
            (PathBuf::from("llms.txt"), "- [Intro](/book/guide/intro.md)\n- [Old](/book/old.md)\n".to_string()),
        ];
        let present = [
            PathBuf::from("guide/deep.md"),
            PathBuf::from("a.png"),
            PathBuf::from("guide/intro.md"),
        ];
        let broken = broken_links(&written, "/book/", |target| {
            present.iter().any(|p| p == target)
        });
        assert_eq!(
            broken,
            vec![
                (PathBuf::from("guide/intro.md"), "missing.md".to_string()),
                (PathBuf::from("guide/intro.md"), "../nope.md".to_string()),
                (PathBuf::from("llms.txt"), "/book/old.md".to_string()),
            ]
        );
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
