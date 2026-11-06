# Frontmatter

This chapter's source opens with:

```yaml
---
description: Frontmatter keys, and which ones reach the output
tags:
  - example
  - frontmatter
---
```

- `description` is added to this chapter's line in `llms.txt`.
- `tags` is allowed and removed. It appears nowhere in the HTML or llms output.
- The block itself is removed from the HTML page by `strip-frontmatter` and from the Markdown output by `mdbook-llms`.
