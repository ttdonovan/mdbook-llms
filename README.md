# mdbook-llms

[![crates.io](https://img.shields.io/crates/v/mdbook-llms.svg)](https://crates.io/crates/mdbook-llms)
[![docs.rs](https://img.shields.io/docsrs/mdbook-llms)](https://docs.rs/mdbook-llms)

An [mdBook](https://rust-lang.github.io/mdBook/) 0.5 renderer that follows the
[llms.txt](https://llmstxt.org/) convention. It writes:

- `llms.txt`: an index of every chapter, grouped by SUMMARY.md parts
- `llms-full.txt`: every chapter in reading order, in one file
- `<chapter>.md`: each chapter's Markdown at the path its HTML would use

It also includes `mdbook-strip-frontmatter`, a preprocessor that keeps YAML
frontmatter out of the HTML pages.

Output comes from mdBook's preprocessed book, so `{{#include}}` and friends are
already expanded.

## Install

```sh
cargo install --locked mdbook-llms
```

From a local checkout, use `cargo install --path .` instead.

## Usage

To generate only the llms files, list only this output. mdBook then skips HTML,
CSS and JS completely, and the files land directly in `book/`:

```toml
[output.llms]
site-url = "https://example.com/my-book"
```

To publish alongside the site, keep both. The files go into `book/html/`, next
to the HTML:

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

With `pages = false`, the links only resolve if the HTML site is published too.

## Optional files

The [llms.txt convention](https://llmstxt.org/) ends the index with an
`## Optional` section for what a reader may skip. List each file under
`[[output.llms.optional]]`:

```toml
[[output.llms.optional]]
path = "llms-full.txt"
description = "Every chapter in one file"

[[output.llms.optional]]
path = "data/model.json"
title = "model.json"
description = "The whole model as JSON"
```

`path` is under `src/`, and the file is copied to the same path in the output;
`llms-full.txt` is the one exception, since the renderer writes it. `title`
defaults to the file name. With `sizes = true`, each entry shows its size too.
A listed file that does not exist fails the build.

## Frontmatter

mdBook does not understand frontmatter, so its HTML renderer would print the
block on the page. This crate also installs a preprocessor that removes it.
Add this whenever HTML is enabled:

```toml
[preprocessor.strip-frontmatter]
```

It steps aside for the `llms` renderer, which needs to read the block, so no
`renderers` setting is required. For llms-only books you can leave it out.

A `description` adds text to that chapter's `llms.txt` entry. Other keys, such
as `tags`, are allowed and removed from all output:

```markdown
---
description: How to install the CLI
tags:
  - setup
---

# Install
```

A block counts as frontmatter only if it opens the file with `---`, closes with
`---`, and every line in between looks like YAML. A chapter that simply starts
with a horizontal rule is left alone.

## Behaviour

- Links between chapters are rewritten so they work from `llms-full.txt`, and
  `README.md` links follow the chapter to its published `index.md`.
- Draft chapters have no page, so they are skipped and their children move up.
- Standalone builds empty the output directory first, as the HTML renderer does.
- Code lines mdBook hides in HTML (`# ` in Rust, `hidelines` prefixes elsewhere)
  are kept in the llms output with the marker removed, so code stays complete.
- When merged into the HTML output, each HTML page gets `<link>` tags pointing
  at its `.md` file and at `llms.txt`, including `index.html` and `print.html`.
  Set `discovery-links = false` to leave the HTML untouched.
- Relative links in `llms.txt` and the per-chapter `.md` files are checked
  after the build, and each missing target prints a
  `warning: mdbook-llms: broken link in <file>: <link>` line. Anchors and
  external URLs are not checked, and the build does not fail.

## Known limitations

- Requires mdBook 0.5.
- mdBook publishes a `README.md` chapter as `index.html` but does not rewrite
  links to it, so they point at a missing `README.html` in the HTML site. Only
  the llms output fixes them.

## Agent skill

[`skills/mdbook-llms/SKILL.md`](skills/mdbook-llms/SKILL.md) is an agent skill
for setting up and checking this renderer in a book. To load it in this
repository, link it into `.claude/skills/` (the link is gitignored):

```sh
mkdir -p .claude/skills
ln -s ../../skills/mdbook-llms .claude/skills/mdbook-llms
```

To use it elsewhere, copy or link the folder into that project's
`.claude/skills/`.
