# Example section

This section exists to show what mdbook-llms produces. Each chapter exercises one feature, and this page shows the resulting `llms.txt` for the whole book.

## What it shows

- **Parts**: `# Guide` and `# Example` in `SUMMARY.md` become `##` headings in `llms.txt`.
- **Section pages**: this file is `example/README.md`, so it is published as `example/index.md`.
- **Descriptions**: each chapter's frontmatter `description` follows its link.
- **Drafts**: `Draft chapter` has no file, so it is left out and [Included files](included.md) moves up a level.

## Generated llms.txt

This is a committed snapshot of `book/html/llms.txt`. `just snapshot` fails if it no longer matches the build.

```text
# mdbook-llms

## Guide

- [Getting started](/mdbook-llms/getting-started.md): What mdbook-llms does and how to try it in a minute
- [Usage](/mdbook-llms/usage.md): Install, configure and the full list of `[output.llms]` options
- [Contributing](/mdbook-llms/contributing.md): Prerequisites, project layout and the just-based dev workflow

## Example

- [Example section](/mdbook-llms/example/index.md): A worked example showing each chapter's source next to the llms.txt it produces
  - [Frontmatter](/mdbook-llms/example/frontmatter.md): Frontmatter keys, and which ones reach the output
  - [Links](/mdbook-llms/example/links.md): How links between chapters are rewritten for each output
  - [Included files](/mdbook-llms/example/included.md): An {{#include}}-built chapter, listed one level up because its parent is a draft
```
