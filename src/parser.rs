use anyhow::{anyhow, Context, Result};
use dyn_fmt::AsStrFormatExt;
use fancy_regex::Regex;
use font_kit::loaders::freetype::Font;
use font_kit::source::SystemSource;
use indoc::indoc;
use pest::iterators::{Pair, Pairs};
use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub type TocEntry = (String, usize);

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct DocumentParser;

#[derive(Debug)]
pub struct Declarations {
    pub include: Vec<String>,
    pub js: Vec<String>,
    pub css: Vec<String>,
    pub fonts: Vec<Font>,
    pub page_width_mm: Option<u64>,
    pub page_height_mm: Option<u64>,
}

#[derive(Debug, Clone)]
enum HtmlToken {
    ElementName { name: String },
    ElementClasses { classes: Vec<String> },
    ElementAttributes { attributes: HashMap<String, String> },
    ElementInlineContent { content: String },
    ElementChildren { children: Vec<HtmlToken> },
    EmptyBlockLine,
    BlockLine { content: String },
}

#[derive(Debug, Default)]
pub struct LexerState {
    page_number: usize,
    pub toc: Vec<TocEntry>,
    template_children: Option<Vec<HtmlToken>>,
    template_classes: Option<Vec<String>>,
    template_attributes: HashMap<String, String>,
}

fn replace_template_attributes(
    content: &str,
    attributes: &HashMap<String, String>,
) -> Result<String> {
    let mut content: String = content.to_string();

    for (key, value) in attributes.iter() {
        let regex = &Regex::new(&format!(r"(?!\}})\{{{}\}}(?!\}})", key))?;
        content = regex.replace(&content, value).to_string();
    }

    Ok(content)
}

impl DocumentParser {
    pub fn get_declarations(mut pairs: Pairs<Rule>) -> Result<Declarations> {
        let mut include: Vec<String> = Vec::new();
        let mut js: Vec<String> = Vec::new();
        let mut css: Vec<String> = Vec::new();
        let mut fonts: Vec<Font> = Vec::new();
        let mut page_width_mm: Option<u64> = None;
        let mut page_height_mm: Option<u64> = None;

        for pair in pairs.next().unwrap().into_inner() {
            match pair.as_rule() {
                Rule::declaration => {
                    let mut iterator = pair.into_inner();
                    let declaration_key_pair = iterator.next().unwrap();
                    let declaration_value_pair = iterator.next().unwrap();

                    if declaration_key_pair.as_rule() != Rule::declaration_key
                        || declaration_value_pair.as_rule() != Rule::declaration_value
                    {
                        return Err(anyhow!("Declaration pairs out of order"));
                    }

                    let declaration_key = declaration_key_pair.as_span().as_str();
                    let declaration_value = declaration_value_pair.as_span().as_str();

                    match declaration_key {
                        "include" => include.push(declaration_value.to_string()),
                        "page-width" => {
                            page_width_mm = Some(
                                declaration_value
                                    .parse::<u64>()
                                    .context("The page-width value is not of u64 value")?,
                            )
                        }
                        "page-height" => {
                            page_height_mm = Some(
                                declaration_value
                                    .parse::<u64>()
                                    .context("The page-height value is not of u64 value")?,
                            )
                        }
                        "font" => {
                            fonts.push(
                                SystemSource::new()
                                    .select_by_postscript_name(declaration_value)
                                    .context("Could not find the selected font")?
                                    .load()?,
                            );
                        }
                        "js" => js.push(declaration_value.to_string()),
                        "css" => css.push(declaration_value.to_string()),
                        _ => {
                            return Err(anyhow!(format!(
                                "The declaration key '{}' is unexpected",
                                declaration_key
                            )))
                        }
                    }
                }
                Rule::block => {}
                Rule::EOI => {}
                _ => return Err(anyhow!(format!("Unexpected document rule: {:?}", pair))),
            }
        }

        Ok(Declarations {
            include,
            js,
            css,
            fonts,
            page_width_mm,
            page_height_mm,
        })
    }

    pub fn include_linked_files(declarations: &Declarations, temporary_dir: &Path) -> Result<()> {
        for file in &declarations.include {
            fs::copy(
                file,
                &temporary_dir.join(Path::new(file).file_name().unwrap()),
            )
            .context(format!("Failed to include '{}'", file))?;
        }

        if !temporary_dir.join("fonts").exists() {
            fs::create_dir(temporary_dir.join("fonts"))
                .context("Failed to create directory fonts")?;
        }

        for font in &declarations.fonts {
            fs::write(
                &temporary_dir.join(format!(
                    "fonts/{}",
                    font.postscript_name()
                        .unwrap()
                        .replace(' ', "-")
                        .to_lowercase()
                )),
                font.copy_font_data().unwrap().as_ref(),
            )
            .context(format!(
                "Failed to add font '{}'",
                font.postscript_name().unwrap()
            ))?;
        }

        Ok(())
    }

    pub fn generate_html(
        declarations: &Declarations,
        lex_state: &mut LexerState,
        pairs: Pairs<Rule>,
    ) -> Result<String> {
        let html_tokens = Self::lex_html_document(lex_state, pairs)?;
        let html_body = Self::generate_html_body(&html_tokens, 4, false, "")?;

        let mut warnings: Vec<railwind::warning::Warning> = Vec::new();
        let generated_css = railwind::parse_to_string(
            railwind::Source::String(html_body.clone(), railwind::CollectionOptions::Html),
            false,
            &mut warnings,
        );

        let html = indoc! {"
            <!DOCTYPE html>
            <html>
              <head>
                <style>
                  * {{
                    margin: 0;
                  }}

                  .page {{
                    width: {}mm;
                    height: {}mm;
                    overflow: hidden;
                    page-break-after: always;
                    counter-increment: page-number;
                  }}

                  .page-number::before {{
                    content: counter(page-number);
                  }}
                  {}
                  {}
                </style>
                {}
              </head>
              <body>{}
              {}
              </body>
            </html>
        "}
        .format(&[
            // default: A4
            declarations.page_width_mm.unwrap_or(210).to_string(),
            declarations.page_height_mm.unwrap_or(297).to_string(),
            declarations
                .fonts
                .iter()
                .map(|font| {
                    format!(
                        "@font-face {{ font-family: {0}; src: url(fonts/{0}); }} .font-{0} {{ font-family: {0}; }}",
                        font.postscript_name().unwrap().replace(' ', "-").to_lowercase()
                    )
                })
                .intersperse(String::from("\n      "))
                .collect(),
            generated_css
                .replace("\n\n", "\n\n      ")
                .replace(";\n", ";\n    ")
                .replace("{\n", "{\n    ")
                .replace('}', "  }"),
            declarations
                .css
                .iter()
                .map(|src| format!("<link rel=\"stylesheet\" href=\"{}\" />", src))
                .intersperse(String::from("\n    "))
                .collect(),
            html_body,
            declarations
                .js
                .iter()
                .map(|src| format!("<script src=\"{}\"></script>", src))
                .intersperse(String::from("\n  "))
                .collect(),
        ]);

        Ok(html)
    }

    fn generate_html_body(
        tokens: &Vec<HtmlToken>,
        indentation: usize,
        children: bool,
        element_name: &str,
    ) -> Result<String> {
        let mut html = String::new();
        let mut element_unclosed = false;
        let mut element_name = element_name;
        let mut following_block_line = false;

        for token in tokens {
            match token {
                HtmlToken::ElementName { name } => {
                    if element_unclosed {
                        html.push_str(" />");
                    }

                    html.push_str(&format!("\n{}<{}", " ".repeat(indentation), name));

                    element_name = name;
                    element_unclosed = true;
                    following_block_line = false;
                }
                HtmlToken::ElementClasses { classes } => {
                    html.push_str(&format!(" class=\"{}\"", classes.join(" ")));
                }
                HtmlToken::ElementAttributes { attributes } => {
                    for (key, value) in attributes.iter() {
                        html.push_str(&format!(" {}=\"{}\"", key, value));
                    }
                }
                HtmlToken::ElementInlineContent { content } => {
                    html.push_str(&format!(">{}</{}>", content, element_name));

                    element_unclosed = false;
                }
                HtmlToken::ElementChildren { children } => {
                    html.push('>');
                    html.push_str(&Self::generate_html_body(
                        children,
                        indentation + 2,
                        true,
                        element_name,
                    )?);
                    html.push_str(&format!("\n{}</{}>", " ".repeat(indentation), element_name));

                    element_unclosed = false;
                }
                HtmlToken::BlockLine { content } => {
                    if following_block_line && element_name != "pre" {
                        html.push_str("<br />");
                    } else {
                        following_block_line = true;
                    }

                    html.push('\n');

                    if element_name != "pre" {
                        html.push_str(&" ".repeat(indentation));
                    }

                    html.push_str(content);
                }
                HtmlToken::EmptyBlockLine => {
                    if element_name != "pre" {
                        html.push_str("<br />");
                    }

                    html.push('\n')
                } // _ => return Err(anyhow!(format!("Unexpected html token: {:?}", token))),
            }
        }

        if element_unclosed && !children {
            html.push_str(" />");
        }

        Ok(html)
    }

    fn lex_html_document(
        lex_state: &mut LexerState,
        mut pairs: Pairs<Rule>,
    ) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();

        for pair in pairs.next().unwrap().into_inner() {
            match pair.as_rule() {
                Rule::block => html.extend(Self::lex_html_block(lex_state, pair.into_inner())?),
                Rule::declaration => {}
                Rule::EOI => {}
                _ => return Err(anyhow!(format!("Unexpected document rule: {:?}", pair))),
            }
        }

        Ok(html)
    }

    fn lex_html_block(lex_state: &mut LexerState, pairs: Pairs<Rule>) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::block_element => html.extend(Self::lex_html_block_element(lex_state, pair)?),
                Rule::block_template => {
                    html.extend(Self::lex_html_block_template(lex_state, pair)?)
                }
                Rule::block_content_line => {
                    html.push(HtmlToken::BlockLine {
                        content: replace_template_attributes(
                            pair.as_span().as_str(),
                            &lex_state.template_attributes,
                        )?,
                    });
                }
                Rule::block_content_empty_line => {
                    html.push(HtmlToken::EmptyBlockLine);
                }
                _ => return Err(anyhow!(format!("Unexpected block rule: {:?}", pair))),
            }
        }

        Ok(html)
    }

    fn lex_html_block_children(
        lex_state: &mut LexerState,
        pairs: Pairs<Rule>,
    ) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::block => html.extend(Self::lex_html_block(lex_state, pair.into_inner())?),
                Rule::ellipsis => html.extend(lex_state.template_children.clone().unwrap()),
                _ => {
                    return Err(anyhow!(format!(
                        "Unexpected block children rule: {:?}",
                        pair
                    )))
                }
            }
        }

        Ok(html)
    }

    fn lex_html_block_element(
        lex_state: &mut LexerState,
        element_pair: Pair<Rule>,
    ) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();
        let mut toc = false;

        for pair in element_pair.into_inner() {
            match pair.as_rule() {
                Rule::block_element_name => {
                    html.push(HtmlToken::ElementName {
                        name: pair.as_span().as_str().to_string(),
                    });

                    toc = false;
                }
                Rule::block_element_classes => {
                    let mut classes: Vec<String> = Vec::new();

                    for class_pair in pair.into_inner() {
                        match class_pair.as_rule() {
                            Rule::block_element_class => {
                                classes.push(class_pair.as_span().as_str().to_string())
                            }
                            Rule::extend_classes => {
                                classes.extend(lex_state.template_classes.clone().unwrap())
                            }
                            _ => return Err(anyhow!("Unexpected element_classes rule")),
                        }
                    }

                    if classes.iter().any(|class| class.as_str() == "toc") {
                        toc = true;
                    }

                    if classes.iter().any(|class| class.as_str() == "page") {
                        lex_state.page_number += 1;
                    }

                    html.push(HtmlToken::ElementClasses { classes });
                }
                Rule::block_element_attributes => {
                    let mut attributes: HashMap<String, String> = HashMap::new();

                    for attribute_pair in pair.into_inner() {
                        if attribute_pair.as_rule() != Rule::attribute {
                            return Err(anyhow!("Unexpected block_element_attributes rule"));
                        }

                        let mut iterator = attribute_pair.into_inner();
                        let attribute_key_pair = iterator.next().unwrap();
                        let attribute_value_pair = iterator.next().unwrap();

                        if attribute_key_pair.as_rule() != Rule::attribute_key
                            || attribute_value_pair.as_rule() != Rule::attribute_value
                        {
                            return Err(anyhow!("Attribute pairs out of order"));
                        }

                        attributes.insert(
                            attribute_key_pair.as_span().as_str().to_string(),
                            attribute_value_pair
                                .as_span()
                                .as_str()
                                .replace("\\\"", "&quot;"),
                        );
                    }

                    html.push(HtmlToken::ElementAttributes { attributes });
                }
                Rule::block_element_content => {
                    let content = replace_template_attributes(
                        pair.as_span().as_str(),
                        &lex_state.template_attributes,
                    )?;

                    if toc {
                        lex_state.toc.push((content.clone(), lex_state.page_number))
                    }

                    html.push(HtmlToken::ElementInlineContent { content });
                }
                Rule::block_children => {
                    html.push(HtmlToken::ElementChildren {
                        children: Self::lex_html_block_children(lex_state, pair.into_inner())?,
                    });
                }
                _ => {
                    return Err(anyhow!(format!(
                        "Unexpected block element rule: {:?}",
                        pair
                    )))
                }
            }
        }

        Ok(html)
    }

    fn lex_html_block_template(
        lex_state: &mut LexerState,
        template_pair: Pair<Rule>,
    ) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();
        let mut template_path = String::new();
        let mut template_content = String::new();
        let mut template_children: Vec<HtmlToken> = Vec::new();
        let mut template_classes: Vec<String> = Vec::new();
        let mut template_attributes: HashMap<String, String> = HashMap::new();

        for pair in template_pair.into_inner() {
            match pair.as_rule() {
                Rule::block_template_name => {
                    template_path = pair.as_span().as_str().replace('-', "/");
                    let mut content: Option<String> = None;

                    for directory in &[
                        "./",
                        "/usr/share/twml/templates/",
                        "~/.config/twml/templates/",
                    ] {
                        content = fs::read_to_string(
                            PathBuf::from(directory)
                                .join(&template_path)
                                .with_extension("twml"),
                        )
                        .ok();

                        if content.is_some() {
                            break;
                        }
                    }

                    if content.is_none() {
                        return Err(anyhow!(format!(
                            "Failed to read template '{}'",
                            template_path
                        )));
                    }

                    template_content = content.unwrap();
                }
                Rule::block_template_classes => {
                    for class_pair in pair.into_inner() {
                        match class_pair.as_rule() {
                            Rule::block_template_class => {
                                template_classes.push(class_pair.as_span().as_str().to_string());
                            }
                            Rule::extend_classes => {
                                template_classes.extend(lex_state.template_classes.clone().unwrap())
                            }
                            _ => return Err(anyhow!("Unexpected template_classes rule")),
                        }
                    }
                }
                Rule::block_template_attributes => {
                    for attribute_pair in pair.into_inner() {
                        if attribute_pair.as_rule() != Rule::attribute {
                            return Err(anyhow!("Unexpected block_template_attributes rule"));
                        }

                        let mut iterator = attribute_pair.into_inner();
                        let attribute_key_pair = iterator.next().unwrap();
                        let attribute_value_pair = iterator.next().unwrap();

                        if attribute_key_pair.as_rule() != Rule::attribute_key
                            || attribute_value_pair.as_rule() != Rule::attribute_value
                        {
                            return Err(anyhow!("Attribute pairs out of order"));
                        }

                        template_attributes.insert(
                            attribute_key_pair.as_span().as_str().to_string(),
                            attribute_value_pair
                                .as_span()
                                .as_str()
                                .replace("\\\"", "&quot;"),
                        );
                    }
                }
                Rule::block_children => {
                    template_children
                        .extend(Self::lex_html_block_children(lex_state, pair.into_inner())?);
                }
                Rule::block_template_content => {
                    template_children.push(HtmlToken::BlockLine {
                        content: replace_template_attributes(
                            pair.as_span().as_str(),
                            &lex_state.template_attributes,
                        )?,
                    });
                }
                _ => {
                    return Err(anyhow!(format!(
                        "Unexpected block template rule: {:?}",
                        pair
                    )))
                }
            }
        }

        let mut inner_lex_state = LexerState {
            page_number: lex_state.page_number,
            toc: Vec::new(),
            template_children: Some(template_children),
            template_classes: Some(template_classes),
            template_attributes,
        };

        html.extend(Self::lex_html_document(
            &mut inner_lex_state,
            DocumentParser::parse(Rule::document, &template_content).context(format!(
                "Failed to interpret the provided template '{}'",
                template_path
            ))?,
        )?);

        lex_state.page_number = inner_lex_state.page_number;
        lex_state.toc.extend(inner_lex_state.toc);

        Ok(html)
    }
}
