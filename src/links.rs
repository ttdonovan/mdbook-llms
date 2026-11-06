//! Links: rewriting them for each output, and checking the results.

use crate::{Page, apply_edits, markdown_options, page_url};
use mdbook_markdown::pulldown_cmark::{Event, Parser, Tag};
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

/// Every way a chapter can be addressed, mapped to its published path. Authors
/// link to `README.md`, but mdBook publishes that chapter as `index.md`.
pub fn link_targets(pages: &[Page]) -> HashMap<PathBuf, PathBuf> {
    let mut targets = HashMap::new();
    for page in pages {
        targets.insert(page.path.to_path_buf(), page.path.to_path_buf());
        if let Some(source) = &page.chapter.source_path {
            targets.insert(source.clone(), page.path.to_path_buf());
        }
    }
    targets
}

#[derive(Clone, Copy)]
pub enum LinkStyle<'a> {
    /// Stays relative to the chapter's own file. Used for per-page Markdown.
    Relative,
    /// Must survive being read outside the chapter's directory.
    Absolute { base: &'a str, extension: &'a str },
}

/// Points links between chapters at their published locations. Uses a real
/// Markdown parser, so anything that looks like a link inside code is left be.
pub fn rewrite_links(
    body: &str,
    chapter: &Path,
    links: &HashMap<PathBuf, PathBuf>,
    style: LinkStyle,
) -> String {
    let parser = Parser::new_ext(body, markdown_options());

    let mut edits: Vec<(Range<usize>, String)> = Vec::new();
    for (_, definition) in parser.reference_definitions().iter() {
        if let Some(new) = resolve(&definition.dest, chapter, links, style)
            && let Some(at) = find_in(body, definition.span.clone(), &definition.dest, false)
        {
            edits.push((at, new));
        }
    }
    for (event, span) in parser.into_offset_iter() {
        let Event::Start(Tag::Link { dest_url, .. }) = event else {
            continue;
        };
        // Search backwards: the link text could contain the same string.
        if let Some(new) = resolve(&dest_url, chapter, links, style)
            && let Some(at) = find_in(body, span, &dest_url, true)
        {
            edits.push((at, new));
        }
    }
    apply_edits(body, edits)
}

/// Where `needle` sits inside `span` of `body`, searching from the end if asked.
fn find_in(body: &str, span: Range<usize>, needle: &str, last: bool) -> Option<Range<usize>> {
    let slice = body.get(span.clone())?;
    let offset = if last {
        slice.rfind(needle)?
    } else {
        slice.find(needle)?
    };
    let start = span.start.checked_add(offset)?;
    Some(start..start.checked_add(needle.len())?)
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
    let is_markdown = Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));
    if !is_markdown || path.starts_with('/') || path.contains("://") {
        return None;
    }
    let dir = chapter.parent().unwrap_or_else(|| Path::new(""));
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

/// Relative links in the generated files whose target does not exist.
/// Anchors and external URLs are not checked.
pub fn broken_links(
    written: &[(PathBuf, String)],
    base: &str,
    exists: impl Fn(&Path) -> bool,
) -> Vec<(PathBuf, String)> {
    let mut broken = Vec::new();
    // One report per missing file, however many anchors point into it.
    let mut seen: Vec<(&PathBuf, PathBuf)> = Vec::new();
    for (file, text) in written {
        let from = file.parent().unwrap_or_else(|| Path::new(""));
        for event in Parser::new_ext(text, markdown_options()) {
            let Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) = event
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
}
