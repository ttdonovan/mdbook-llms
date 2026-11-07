# Included files

mdbook-llms reads the book after mdBook's links preprocessor has run, so `{{#include}}` is already expanded in every output. This is this book's own `book.toml`, included from outside `src/`:

```toml
[book]
title = "mdbook-llms"
src = "src"

[preprocessor.strip-frontmatter]

[preprocessor.mermaid]
command = "mdbook-mermaid"

[output.html]
site-url = "/mdbook-llms/"
additional-js = ["mermaid.min.js", "mermaid-init.js"]

[output.llms]
```

## Including Rust source with anchors

An include can pull a named region out of a source file. `src/lib.rs` marks the renderer's config with `// ANCHOR: config` and `// ANCHOR_END: config`, and this page includes it with:

```text
{{#include ../../../src/lib.rs:config}}
```

mdBook drops the anchor lines, so the result is the real definition of every `[output.llms]` option and its default:

```rust
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
    /// Files listed under `## Optional` in `llms.txt`. Not `optional`, which
    /// mdBook reads under every output as a bool of its own.
    pub optional_files: Vec<Optional>,
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
            optional_files: Vec::new(),
        }
    }
}
```

Anchors move with the code, unlike line ranges such as `lib.rs:15:40`. Because the snapshot test covers this page, a change to `Config` also shows up as a docs change.

## Drafts

In `SUMMARY.md` this chapter sits under `Draft chapter`, which has no file. Drafts get no page, so in `llms.txt` this chapter is listed one level up instead of disappearing.
