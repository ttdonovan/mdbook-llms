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

In `SUMMARY.md` this chapter sits under `Draft chapter`, which has no file. Drafts get no page, so in `llms.txt` this chapter is listed one level up instead of disappearing.
