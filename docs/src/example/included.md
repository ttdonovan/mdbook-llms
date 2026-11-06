---
description: An \{{#include}}-built chapter, listed one level up because its parent is a draft
---

# Included files

mdbook-llms reads the book after mdBook's links preprocessor has run, so `\{{#include}}` is already expanded in every output. This is this book's own `book.toml`, included from outside `src/`:

```toml
{{#include ../../book.toml}}
```

## Including Rust source with anchors

An include can pull a named region out of a source file. `src/lib.rs` marks the renderer's config with `// ANCHOR: config` and `// ANCHOR_END: config`, and this page includes it with:

```text
\{{#include ../../../src/lib.rs:config}}
```

mdBook drops the anchor lines, so the result is the real definition of every `[output.llms]` option and its default:

```rust
{{#include ../../../src/lib.rs:config}}
```

Anchors move with the code, unlike line ranges such as `lib.rs:15:40`. Because the snapshot test covers this page, a change to `Config` also shows up as a docs change.

## Drafts

In `SUMMARY.md` this chapter sits under `Draft chapter`, which has no file. Drafts get no page, so in `llms.txt` this chapter is listed one level up instead of disappearing.
