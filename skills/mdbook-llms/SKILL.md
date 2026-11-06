---
name: mdbook-llms
description: Add llms.txt, llms-full.txt and per-chapter Markdown output to an mdBook 0.5 book with the mdbook-llms renderer. Use when asked to make an mdBook readable by LLMs, add llms.txt to a book, configure [output.llms], or debug its output.
---

# mdbook-llms

mdbook-llms is an mdBook 0.5 renderer. It writes `llms.txt` (an index of chapters), `llms-full.txt` (every chapter in one file) and a `.md` file per chapter. It ships with `mdbook-strip-frontmatter`, a preprocessor that keeps YAML frontmatter out of the HTML pages.

For every option and its default, read [USAGE.md](https://github.com/ttdonovan/mdbook-llms/blob/main/USAGE.md). This skill covers the steps and the gotchas.

## Steps

1. Check `mdbook --version` reports 0.5.x. Older versions are not supported.
2. Install both binaries with `cargo install --locked --git https://github.com/ttdonovan/mdbook-llms` (or `cargo install --path .` from a checkout), and confirm `mdbook-llms` and `mdbook-strip-frontmatter` are on `PATH`.
3. Decide between the two layouts in `book.toml`:
   - Alongside the HTML site (usual case): keep `[output.html]`, add `[output.llms]` and `[preprocessor.strip-frontmatter]`. Files go into `book/html/`.
   - llms output only: list only `[output.llms]`. Files go into `book/`, and the strip-frontmatter preprocessor is not needed.
4. Set `site-url` under `[output.llms]` if links in `llms.txt` must be absolute. Without it, `output.html.site-url` is used, and without either, links are relative to the output root.
5. Optionally add a `description` to chapters through frontmatter. It is appended to that chapter's line in `llms.txt`:

   ```markdown
   ---
   description: How to install the CLI
   ---

   # Install
   ```

6. Run `mdbook build` and check the result (see Verify).

## Gotchas

- **Missing preprocessor**: if HTML is enabled without `[preprocessor.strip-frontmatter]`, frontmatter blocks are printed on the HTML pages. No `renderers` setting is needed; it already skips the llms renderer, which needs the frontmatter.
- **Frontmatter detection**: a block only counts if it opens the file with `---`, closes with `---`, and every line between looks like YAML. Anything else is treated as content.
- **Links to README.md chapters**: mdBook publishes `README.md` as `index.html` but does not rewrite links to it, so the HTML site links to a missing `README.html`. mdbook-llms fixes these links in its own output only. Avoid linking to `README.md` chapters if the HTML site matters.
- **mdbook serve**: it forces `output.html.site-url` to `/`, so when `[output.llms]` has no `site-url`, served llms links become root-relative (`/usage.md`). Use `mdbook build` for output you check or publish, and do not run serve and build into the same directory at once.
- **Hidden Rust lines**: lines hidden with `# ` in Rust code blocks appear in full in the Markdown output.
- **Draft chapters**: chapters with no file (`[Title]()`) get no entry, and their children move up one level in `llms.txt`.
- **merge-into-html**: when HTML also runs, the renderer writes into `book/html/` and fails if no HTML output exists there. mdBook still creates an empty `book/llms/` directory, which can be ignored.

## Verify

- `llms.txt` exists in the output directory and lists every non-draft chapter, grouped under the `SUMMARY.md` part titles (or `## Contents` if there are none).
- Links in `llms.txt` use the expected prefix, and chapter pages exist at those paths (`README.md` chapters appear as `index.md`).
- No HTML page shows a frontmatter block.
- `{{#include}}` directives are expanded in `llms-full.txt`.
