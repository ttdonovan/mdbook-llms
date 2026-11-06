# Contributing

## Prerequisites

- Rust (edition 2024)
- [just](https://github.com/casey/just)
- [mdBook](https://rust-lang.github.io/mdBook/) 0.5
- [mdbook-mermaid](https://github.com/badboy/mdbook-mermaid) 0.17
- [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit)

`just setup` installs mdBook and mdbook-mermaid at the versions the docs are tested with.

## Layout

- `src/lib.rs`: config, link rewriting, frontmatter parsing and output writing.
- `src/main.rs`: the `mdbook-llms` renderer binary.
- `src/bin/mdbook-strip-frontmatter.rs`: the frontmatter preprocessor binary.
- `docs/`: this book, built with the renderer from the working tree.

## Workflow

Run `just` to list recipes. Before opening a pull request, run:

```sh
just fmt
just check
```

`check` runs `fmt-check`, `clippy` (warnings are errors), `test` and `audit`.

## Releases

Releases are made by [release-plz](https://release-plz.dev/) from [Conventional Commits](https://www.conventionalcommits.org/), so prefix commit messages with `feat:`, `fix:`, `docs:` and so on, and mark breaking changes with `!` (e.g. `feat!:`). On each push to `main`, release-plz opens or updates a release PR that bumps the version and updates `CHANGELOG.md`. Merging it publishes the new version to crates.io through [trusted publishing](https://crates.io/docs/trusted-publishing), tags `vX.Y.Z` and creates a GitHub Release.

## Building the docs

```sh
just book
```

This builds the crate, puts `target/debug` first on `PATH` so mdBook uses the local binaries, and writes the site and llms files to `docs/book/`. Use `just serve` to preview the HTML.

`docs/snapshots/` holds the expected llms output for this book: `llms.txt`, `llms-full.txt` and every per-chapter `.md` file. `just snapshot` builds into `target/book-snapshot/` and fails if any of them differ. After an intended change to the renderer or the docs, run `just snapshot-update` and review the diff.
