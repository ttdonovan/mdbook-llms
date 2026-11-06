---
description: A worked example showing each chapter's source next to the llms.txt it produces
---

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
{{#include ../../snapshots/llms.txt}}
```
