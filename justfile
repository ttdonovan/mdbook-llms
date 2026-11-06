default:
    @just --list

build:
    cargo build

test:
    cargo test

fmt:
    cargo fmt

fmt-check:
    cargo fmt --check

clippy:
    cargo clippy --all-targets -- -D warnings

audit:
    cargo audit

install:
    cargo install --path .

clean:
    cargo clean

# Run all checks
check: fmt-check clippy test audit

# Build the docs with the local renderer
book: build
    PATH="{{justfile_directory()}}/target/debug:$PATH" mdbook build docs

# Preview the docs with the local renderer
serve: build
    PATH="{{justfile_directory()}}/target/debug:$PATH" mdbook serve docs

# Install the mdBook tools the docs need
setup:
    cargo install --locked mdbook@0.5.2 mdbook-mermaid@0.17.1

# Own output dir, so a running `mdbook serve` can't race it or set site-url
snapshot_dir := justfile_directory() / "target/book-snapshot"

# Build the docs and collect only the llms output into snapshot_dir/out
snapshot-build: build
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf "{{snapshot_dir}}"
    PATH="{{justfile_directory()}}/target/debug:$PATH" mdbook build docs -d "{{snapshot_dir}}/book"
    mkdir -p "{{snapshot_dir}}/out"
    cd "{{snapshot_dir}}/book/html"
    find . -type f \( -name '*.md' -o -name 'llms.txt' -o -name 'llms-full.txt' \) | tar -cf - -T - | tar -xf - -C "{{snapshot_dir}}/out"
    grep -roE '<link rel="(alternate|describedby)"[^>]*>' --include='*.html' . | LC_ALL=C sort > "{{snapshot_dir}}/out/discovery-links.txt"

# Fail if any llms output differs from the committed snapshots
snapshot: snapshot-build
    diff -ru docs/snapshots "{{snapshot_dir}}/out"

# Replace the committed snapshots with the current llms output
# Two passes: the example chapter includes snapshots/llms.txt itself
snapshot-update:
    for pass in 1 2; do just snapshot-build && rm -rf docs/snapshots && cp -R "{{snapshot_dir}}/out" docs/snapshots; done
