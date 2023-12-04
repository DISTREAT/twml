use anyhow::{anyhow, Context, Result};
use dyn_fmt::AsStrFormatExt;
use indoc::indoc;
use pest::iterators::{Pair, Pairs};
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct DocumentParser;

#[derive(Debug)]
pub struct Declarations {
    pub include: Vec<String>,
    pub page_width_mm: Option<u64>,
    pub page_height_mm: Option<u64>,
}

impl DocumentParser {
    pub fn get_declarations(mut pairs: Pairs<Rule>) -> Result<Declarations> {
        let mut include: Vec<String> = Vec::new();
        let mut page_width_mm: Option<u64> = None;
        let mut page_height_mm: Option<u64> = None;

        for pair in pairs.next().unwrap().into_inner() {
            match pair.as_rule() {
                Rule::element => {}
                Rule::EOI => {}
                Rule::declarations => {
                    for declaration_pair in pair.into_inner() {
                        if declaration_pair.as_rule() != Rule::declaration {
                            return Err(anyhow!("Unexpected declarations rule"));
                        }

                        let mut iterator = declaration_pair.into_inner();
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
                            _ => {
                                return Err(anyhow!(format!(
                                    "The declaration key '{}' is unexpected",
                                    declaration_key
                                )))
                            }
                        }
                    }
                }
                _ => return Err(anyhow!(format!("Unexpected document rule: {:?}", pair))),
            }
        }

        Ok(Declarations {
            include,
            page_width_mm,
            page_height_mm,
        })
    }

    pub fn generate_html(declarations: &Declarations, pairs: Pairs<Rule>) -> Result<String> {
        let generated_html = Self::generate_html_document(pairs)?;

        let mut warnings: Vec<railwind::warning::Warning> = Vec::new();
        let generated_css = railwind::parse_to_string(
            railwind::Source::String(generated_html.clone(), railwind::CollectionOptions::Html),
            false,
            &mut warnings,
        )
        .replace('\n', "")
        .replace("    ", "");

        let html = indoc! {"
            <!DOCTYPE html>
            <html>
            <head>
              <style>
                .page {{ width: {}mm; height: {}mm; }}
                {}
              </style>
            </head>
            <body>
              {}
            </body>
            </html>
        "}
        .replace('\n', "")
        .replace("  ", "")
        .format(&[
            // default: A4
            declarations.page_width_mm.unwrap_or(210).to_string(),
            declarations.page_height_mm.unwrap_or(297).to_string(),
            generated_css,
            generated_html,
        ]);

        Ok(html)
    }

    fn generate_html_document(mut pairs: Pairs<Rule>) -> Result<String> {
        let mut html = String::new();

        for pair in pairs.next().unwrap().into_inner() {
            match pair.as_rule() {
                Rule::element => html.push_str(&Self::generate_html_element(pair)?),
                Rule::EOI => {}
                Rule::declarations => {}
                _ => return Err(anyhow!(format!("Unexpected document rule: {:?}", pair))),
            }
        }

        Ok(html)
    }

    fn generate_html_children(pairs: Pairs<Rule>) -> Result<String> {
        let mut html = String::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::block => {
                    let mut newline_needed = false;

                    for class_pair in pair.into_inner() {
                        match class_pair.as_rule() {
                            Rule::block_content => {
                                if newline_needed {
                                    html.push_str("<br>");
                                }

                                html.push_str(class_pair.as_span().as_str());
                                newline_needed = true;
                            }
                            Rule::block_newline => {
                                html.push_str("<br>");
                            }
                            _ => {
                                return Err(anyhow!(format!(
                                    "Unexpected block rule: {:?}",
                                    class_pair
                                )))
                            }
                        }
                    }
                }
                Rule::element => {
                    html.push_str(&Self::generate_html_element(pair)?);
                }
                _ => return Err(anyhow!(format!("Unexpected children rule: {:?}", pair))),
            }
        }

        Ok(html)
    }

    fn generate_html_element(element_pair: Pair<Rule>) -> Result<String> {
        let mut html = String::from("<");
        let mut element_name = String::new();

        for pair in element_pair.into_inner() {
            match pair.as_rule() {
                Rule::element_name => {
                    element_name = pair.as_span().as_str().to_owned();
                    html.push_str(&element_name);
                }
                Rule::element_classes => {
                    html.push_str(" class=\"");

                    let mut seperator_needed = false;

                    for class_pair in pair.into_inner() {
                        if class_pair.as_rule() != Rule::element_class_name {
                            return Err(anyhow!("Unexpected element_classes rule"));
                        }

                        if seperator_needed {
                            html.push(' ');
                        }

                        html.push_str(class_pair.as_span().as_str());
                        seperator_needed = true;
                    }
                    html.push('"');
                }
                Rule::element_attributes => {
                    for attribute_pair in pair.into_inner() {
                        if attribute_pair.as_rule() != Rule::element_attribute {
                            return Err(anyhow!("Unexpected element_attributes rule"));
                        }

                        let mut iterator = attribute_pair.into_inner();
                        let attribute_key_pair = iterator.next().unwrap();
                        let attribute_value_pair = iterator.next().unwrap();

                        if attribute_key_pair.as_rule() != Rule::element_attribute_key
                            || attribute_value_pair.as_rule() != Rule::element_attribute_value
                        {
                            return Err(anyhow!("Attribute pairs out of order"));
                        }

                        html.push(' ');
                        html.push_str(attribute_key_pair.as_span().as_str());
                        html.push_str("=\"");
                        html.push_str(
                            &attribute_value_pair
                                .as_span()
                                .as_str()
                                .replace("\\\"", "&quot;"),
                        );
                        html.push('"');
                    }
                }
                Rule::element_content => {
                    let content = pair.as_span().as_str();
                    html.push_str(&format!(">{}</{}>", content, &element_name))
                }
                Rule::children => {
                    html.push('>');
                    html.push_str(&Self::generate_html_children(pair.into_inner())?);
                    html.push_str(&format!("</{}>", &element_name))
                }
                _ => return Err(anyhow!(format!("Unexpected element rule: {:?}", pair))),
            }
        }

        Ok(html)
    }
}
