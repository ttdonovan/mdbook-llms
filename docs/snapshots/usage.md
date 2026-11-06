# Usage

## Install

```sh
cargo install --locked --git https://github.com/ttdonovan/mdbook-llms
```

From a local checkout, use `cargo install --path .` instead.

This installs two binaries: the `mdbook-llms` renderer and the `mdbook-strip-frontmatter` preprocessor. Both must be on your `PATH` when you run `mdbook build`.

## Configure

To generate only the llms files, list only this output. mdBook then skips HTML, CSS and JS completely, and the files land directly in `book/`:

```toml
[output.llms]
site-url = "https://example.com/my-book"
```

To publish alongside the site, keep both. The files go into `book/html/`, next to the HTML:

```toml
[preprocessor.strip-frontmatter]

[output.html]
site-url = "/my-book/"

[output.llms]
```

To skip HTML for one build without touching `book.toml`:

```sh
MDBOOK_OUTPUT='{"llms":{}}' mdbook build
```

## Options

All go under `[output.llms]`.

| Key | Default | Effect |
|---|---|---|
| `site-url` | `output.html.site-url` | Prefix for links. Without either, links are relative to the output root. |
| `index` | `true` | Write `llms.txt`. |
| `full` | `true` | Write `llms-full.txt`. |
| `pages` | `true` | Write per-chapter `.md` files. When `false`, links point at `.html`. |
| `merge-into-html` | `true` | When HTML also runs, write into its directory rather than `book/llms/`. |

## Frontmatter

A `description` key in a chapter's YAML frontmatter adds text to that chapter's `llms.txt` entry. Other keys are allowed and removed from all output:

```markdown
---
description: How to install the CLI
---

# Install
```

Add `[preprocessor.strip-frontmatter]` whenever HTML is enabled, or the HTML pages will show the block.
