//! The index options, rendered through `run` on a small book.

#![allow(clippy::expect_used)]

use mdbook_renderer::RenderContext;
use mdbook_renderer::book::{Book, BookItem, Chapter};
use mdbook_renderer::config::Config;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

const INTRO: &str = "---\ndescription: Where to start\n---\n\n# Intro\n\nRead this first.\n";

/// A book in a fresh directory: `src/` holds a preamble and a data file.
fn book(name: &str, toml: &str) -> (PathBuf, RenderContext) {
    let root = std::env::temp_dir().join(format!("mdbook-llms-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("src/data")).expect("create src");
    fs::write(
        root.join("src/preamble.md"),
        "Read the intro, then any page.\n",
    )
    .expect("preamble");
    fs::write(root.join("src/data/model.json"), "{\"a\": 1}\n").expect("data");
    let config = Config::from_str(toml).expect("config");
    let items = vec![
        BookItem::PartTitle("Start".to_string()),
        BookItem::Chapter(Chapter::new(
            "Intro",
            INTRO.to_string(),
            "intro.md",
            Vec::new(),
        )),
    ];
    let ctx = RenderContext::new(
        &root,
        Book::new_with_items(items),
        config,
        root.join("book"),
    );
    (root, ctx)
}

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join("book").join(path)).expect("output file")
}

#[test]
fn sizes_a_preamble_and_optional_files() {
    let (root, ctx) = book(
        "all",
        r#"
[book]
title = "Model"

[output.llms]
sizes = true
preamble = "preamble.md"

[[output.llms.optional-files]]
path = "llms-full.txt"
description = "Every page in one file"

[[output.llms.optional-files]]
path = "data/model.json"
title = "model.json"
description = "The model as JSON"
"#,
    );
    mdbook_llms::run(&ctx).expect("render");
    let index = read(&root, "llms.txt");
    let page = read(&root, "intro.md");
    let full = read(&root, "llms-full.txt");
    let expected = format!(
        "# Model\n\nRead the intro, then any page.\n\n## Start\n\n- [Intro](intro.md): Where to start ({})\n\n## Optional\n\n- [llms-full.txt](llms-full.txt): Every page in one file ({})\n- [model.json](data/model.json): The model as JSON (about 100 tokens)\n",
        size(&page),
        size(&full),
    );
    assert_eq!(index, expected);
    assert!(full.starts_with("# Model\n\nRead the intro, then any page.\n\n---"));
    assert_eq!(read(&root, "data/model.json"), "{\"a\": 1}\n");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn without_the_options_the_index_is_as_before() {
    let (root, ctx) = book("none", "[book]\ntitle = \"Model\"\n\n[output.llms]\n");
    mdbook_llms::run(&ctx).expect("render");
    assert_eq!(
        read(&root, "llms.txt"),
        "# Model\n\n## Start\n\n- [Intro](intro.md): Where to start\n"
    );
    assert!(!root.join("book/data").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_missing_optional_file_fails_the_build() {
    let (root, ctx) = book(
        "missing",
        "[book]\ntitle = \"Model\"\n\n[output.llms]\n\n[[output.llms.optional-files]]\npath = \"nope.json\"\n",
    );
    let err = mdbook_llms::run(&ctx).expect_err("a missing file is an error");
    assert!(err.to_string().contains("nope.json"), "{err}");
    let _ = fs::remove_dir_all(&root);
}

/// mdBook reads `optional` and `command` under every output itself, so they
/// must parse beside this renderer's own keys.
#[test]
fn mdbooks_own_output_keys_still_parse() {
    let (root, ctx) = book(
        "mdbook-keys",
        "[book]\ntitle = \"Model\"\n\n[output.llms]\noptional = true\ncommand = \"mdbook-llms\"\n\n[[output.llms.optional-files]]\npath = \"data/model.json\"\n",
    );
    mdbook_llms::run(&ctx).expect("render");
    assert!(read(&root, "llms.txt").contains("## Optional\n\n- [model.json](data/model.json)"));
    let _ = fs::remove_dir_all(&root);
}

fn size(text: &str) -> String {
    let estimate = text.len().div_ceil(4);
    assert!(estimate < 1000, "the test pages are small");
    format!("about {}00 tokens", estimate.div_ceil(100).max(1))
}
