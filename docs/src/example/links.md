---
description: How links between chapters are rewritten for each output
---

# Links

The source of this chapter links to other chapters by their `.md` paths:

- [Usage](../usage.md)
- [Usage options](../usage.md#options)
- [Frontmatter](frontmatter.md)

In the output:

- **HTML** links point at `.html` pages, as usual for mdBook.
- **Per-chapter `.md` files** keep relative `.md` links. A link to a `README.md` chapter is changed to `index.md`, its published path.
- **`llms-full.txt`** has every chapter in one file, so the links are rewritten to work from there.

## Broken links

After the build, mdbook-llms checks the relative links in `llms.txt` and in every chapter's `.md` file. A missing target, such as a typo or a chapter left out of `SUMMARY.md`, prints a warning like this and the build carries on:

```text
warning: mdbook-llms: broken link in example/links.md: nowhere.md
```

Anchors and external URLs are not checked. This repository's `just snapshot` treats these warnings as errors.

## Limitation: links to README.md

mdBook publishes a `README.md` chapter as `index.html`, but its HTML renderer does not rewrite links to it. A link like `[Overview](README.md)` becomes `README.html`, which does not exist, so it is broken in the HTML site.

mdbook-llms changes these links to `index.md` in its own output, but cannot fix the HTML. This book avoids linking to `README.md` chapters for that reason.
