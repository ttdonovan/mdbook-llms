# Usage

## Install

```sh
cargo install --locked mdbook-llms
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
| `discovery-links` | `true` | When merged into the HTML output, add `<link rel="alternate" type="text/markdown">` and `<link rel="describedby">` to each HTML page, as [llms.txt v2](https://llmstxt.org/) recommends. |
| `sizes` | `false` | End each `llms.txt` entry with its file's size, as `(about 1.4k tokens)`: a quarter of its bytes, rounded up to the hundred below a thousand and to a tenth of a thousand above. |
| `preamble` | none | A file under `src/` whose text goes after the title and description of `llms.txt` and `llms-full.txt`. |
| `optional` | none | Files that are not chapters, listed under `## Optional` at the end of `llms.txt` (see below). |

## Optional files

The [llms.txt convention](https://llmstxt.org/) ends the index with an `## Optional` section for what a reader may skip. List each file under `[[output.llms.optional]]`:

```toml
[[output.llms.optional]]
path = "llms-full.txt"
description = "Every chapter in one file"

[[output.llms.optional]]
path = "data/model.json"
title = "model.json"
description = "The whole model as JSON"
```

`path` is under `src/`, and the file is copied to the same path in the output; `llms-full.txt` is the one exception, since the renderer writes it. `title` defaults to the file name. With `sizes = true`, each entry shows its size too. A listed file that does not exist fails the build.

## Frontmatter

A `description` key in a chapter's YAML frontmatter adds text to that chapter's `llms.txt` entry. Other keys are allowed and removed from all output:

```markdown
---
description: How to install the CLI
---

# Install
```

Add `[preprocessor.strip-frontmatter]` whenever HTML is enabled, or the HTML pages will show the block.
