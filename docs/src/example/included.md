---
description: An \{{#include}}-built chapter, listed one level up because its parent is a draft
---

# Included files

mdbook-llms reads the book after mdBook's links preprocessor has run, so `\{{#include}}` is already expanded in every output. This is this book's own `book.toml`, included from outside `src/`:

```toml
{{#include ../../book.toml}}
```

In `SUMMARY.md` this chapter sits under `Draft chapter`, which has no file. Drafts get no page, so in `llms.txt` this chapter is listed one level up instead of disappearing.
