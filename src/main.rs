use anyhow::{Context, Result};
use headless_chrome::browser::LaunchOptions;
use headless_chrome::types::PrintToPdfOptions;
use headless_chrome::Browser;
use parser::Rule;
use pest::Parser;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::exit;
use tempfile::tempdir;

mod parser;

const FACTOR_MM_TO_INCHES: f64 = 25.4;

fn main() -> Result<()> {
    let arguments: Vec<String> = env::args().collect();

    if arguments.len() != 3 {
        println!("Usage: {} <input.twml> <output.pdf>", arguments[0]);
        exit(22);
    }

    let document =
        fs::read_to_string(&arguments[1]).context("Failed to read the input document")?;
    let pairs = parser::DocumentParser::parse(Rule::document, &document)
        .context("Failed to interpret the provided document")?;
    let declarations = parser::DocumentParser::get_declarations(pairs.clone())
        .context("Failed to parse the declarations")?;
    let html = parser::DocumentParser::generate_html(&declarations, pairs)
        .context("Failed to generate html code")?;

    #[cfg(debug_assertions)]
    println!("html: {}", html);

    let temporary_dir = tempdir().context("Failed to create a temporary directory")?;
    let index_path = setup_rendering_env(&declarations, temporary_dir.path(), &html)?;
    export_pdf(&declarations, &index_path, &arguments[2])?;

    Ok(())
}

fn setup_rendering_env(
    declarations: &parser::Declarations,
    temporary_dir: &Path,
    html: &str,
) -> Result<String> {
    let index_path = temporary_dir.join("index.html");
    let mut index_file = fs::File::create(&index_path)?;
    write!(index_file, "{}", html)?;

    for file in &declarations.include {
        fs::copy(
            file,
            &temporary_dir.join(Path::new(file).file_name().unwrap()),
        )
        .context(format!("Failed to include '{}'", file))?;
    }

    Ok(index_path.display().to_string())
}

fn export_pdf(
    declarations: &parser::Declarations,
    index_path: &str,
    output_pdf_path: &str,
) -> Result<()> {
    let browser = Browser::new(LaunchOptions::default())
        .context("Failed to initialize a headless_chrome Browser instance")?;
    let tab = browser.new_tab()?;
    let local_pdf = tab
        .navigate_to(&format!("file://{}", index_path))?
        .wait_until_navigated()?
        .print_to_pdf(Some(PrintToPdfOptions {
            paper_width: Some(
                declarations.page_width_mm.unwrap_or(210) as f64 / FACTOR_MM_TO_INCHES,
            ),
            paper_height: Some(
                declarations.page_height_mm.unwrap_or(297) as f64 / FACTOR_MM_TO_INCHES,
            ),
            margin_top: Some(0.0),
            margin_bottom: Some(0.0),
            margin_left: Some(0.0),
            margin_right: Some(0.0),
            ..PrintToPdfOptions::default()
        }))
        .context("Failed to render the pdf")?;

    fs::write(output_pdf_path, local_pdf).context("Failed to write the output pdf")?;

    Ok(())
}
