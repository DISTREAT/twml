use anyhow::{Context, Result};
use pest::Parser;
use std::env;
use std::fs;
use std::process::exit;
use twml::parser::{DocumentParser, LexerState, Rule};

fn main() -> Result<()> {
    let arguments: Vec<String> = env::args().collect();

    if arguments.len() != 3 {
        println!("Usage: {} <input.twml> <output.html>", arguments[0]);
        exit(22);
    }

    let document =
        fs::read_to_string(&arguments[1]).context("Failed to read the input document")?;
    let pairs = DocumentParser::parse(Rule::document, &document)
        .context("Failed to interpret the provided document")?;
    let declarations = DocumentParser::get_declarations(pairs.clone())
        .context("Failed to parse the declarations")?;
    let mut lex_state = LexerState::default();
    let html = DocumentParser::generate_html(&declarations, &mut lex_state, pairs)
        .context("Failed to generate html code")?;

    fs::write(&arguments[2], html).context("Failed to write the output html")?;

    Ok(())
}
