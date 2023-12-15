use anyhow::{anyhow, Context, Result};
use headless_chrome::browser::LaunchOptions;
use headless_chrome::types::PrintToPdfOptions;
use headless_chrome::Browser;
use lopdf::{Bookmark, Document, Object};
use pest::Parser;
use std::env;
use std::io::{Read, Write};
use std::path::Path;
use std::process::exit;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, fs::File};
use twml::parser::{Declarations, DocumentParser, LexerState, Rule, TocEntry};

const FACTOR_MM_TO_INCHES: f64 = 25.4;

fn main() -> Result<()> {
    let arguments: Vec<String> = env::args().collect();

    if arguments.len() != 3 {
        println!("Usage: {} <input.twml> <output.pdf>", arguments[0]);
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

    let time = SystemTime::now().duration_since(UNIX_EPOCH)?.subsec_nanos();
    let temporary_dir_path = env::temp_dir().join(format!("twml-live-{}", time));
    fs::create_dir(&temporary_dir_path).context("Failed to create a temporary directory")?;

    let index_path = setup_rendering_env(&declarations, &temporary_dir_path, &html)?;
    export_pdf(&declarations, lex_state.toc, &index_path, &arguments[2])?;

    fs::remove_dir_all(&temporary_dir_path).context("Failed to clean up temporary directory")?;

    Ok(())
}

fn setup_rendering_env(
    declarations: &Declarations,
    temporary_dir: &Path,
    html: &str,
) -> Result<String> {
    let index_path = temporary_dir.join("index.html");
    let mut index_file = fs::File::create(&index_path)?;
    write!(index_file, "{}", html)?;

    DocumentParser::include_linked_files(declarations, temporary_dir)?;

    Ok(index_path.display().to_string())
}

fn export_pdf(
    declarations: &Declarations,
    toc: Vec<TocEntry>,
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

    let mut output_pdf = File::create(output_pdf_path).context("Failed to create pdf file")?;

    if !toc.is_empty() {
        modify_pdf_toc(&local_pdf[..], &mut output_pdf, toc)
            .context("Failed to append a pdf outline")?;
    } else {
        output_pdf
            .write_all(&local_pdf[..])
            .context("Failed to write to pdf")?;
    }

    Ok(())
}

fn modify_pdf_toc<R: Read, W: Write>(
    pdf_stream: R,
    output_stream: &mut W,
    toc: Vec<TocEntry>,
) -> Result<()> {
    let mut document = Document::load_from(pdf_stream).context("Failed to read pdf stream")?;
    let pages = document.get_pages();
    let catalog_id = *document
        .objects
        .iter()
        .find(|(_, object)| object.type_name().ok() == Some("Catalog"))
        .context("Failed to find pdf catalog")?
        .0;

    for (name, page_number) in toc {
        document.add_bookmark(
            Bookmark::new(
                name,
                [0.0, 0.0, 0.0],
                0,
                *pages
                    .iter()
                    .nth(page_number - 1)
                    .context("Failed to find bookmarked page")?
                    .1,
            ),
            None,
        );
    }

    if let Some(n) = document.build_outline() {
        if let Ok(Object::Dictionary(ref mut dictionary)) = document.get_object_mut(catalog_id) {
            dictionary.set("Outlines", Object::Reference(n));
        } else {
            return Err(anyhow!("Failed to set outlines reference"));
        }
    } else {
        // unreachable?
    }

    document
        .save_to(output_stream)
        .context("Failed to write to output_stream")?;

    Ok(())
}
