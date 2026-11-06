//! An mdBook renderer for the llms.txt convention. mdBook pipes a JSON
//! `RenderContext` to stdin, and everything else lives in the library.

use anyhow::Result;
use mdbook_renderer::RenderContext;

fn main() -> Result<()> {
    let ctx = RenderContext::from_json(std::io::stdin())?;
    mdbook_llms::run(&ctx)
}
