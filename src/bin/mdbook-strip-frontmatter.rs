//! An mdBook preprocessor that removes YAML frontmatter from chapters, so the
//! HTML renderer does not print it on the page.

use anyhow::Result;
use mdbook_llms::split_frontmatter;
use mdbook_preprocessor::book::BookItem;
use mdbook_preprocessor::parse_input;
use std::env;
use std::io;
use std::process::ExitCode;

/// `mdbook-llms` reads the frontmatter itself, so it has to arrive intact.
const SKIPPED_RENDERER: &str = "llms";

fn main() -> Result<ExitCode> {
    let args: Vec<String> = env::args().skip(1).collect();
    if let [command, renderer] = args.as_slice()
        && command == "supports"
    {
        // mdBook reads a non-zero exit as "not supported".
        return Ok(if renderer == SKIPPED_RENDERER {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        });
    }

    let (_context, mut book) = parse_input(io::stdin())?;
    book.for_each_mut(|item| {
        if let BookItem::Chapter(chapter) = item {
            let (_, body) = split_frontmatter(&chapter.content);
            chapter.content = body.to_string();
        }
    });
    serde_json::to_writer(io::stdout(), &book)?;
    Ok(ExitCode::SUCCESS)
}
