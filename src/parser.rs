use anyhow::{anyhow, Context, Result};
use dyn_fmt::AsStrFormatExt;
use indoc::indoc;
use pest::iterators::{Pair, Pairs};
use pest_derive::Parser;
use std::collections::HashMap;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct DocumentParser;

#[derive(Debug)]
pub struct Declarations {
    pub include: Vec<String>,
    pub page_width_mm: Option<u64>,
    pub page_height_mm: Option<u64>,
}

#[derive(Debug)]
enum HtmlToken {
    ElementName { name: String },
    ElementClasses { classes: Vec<String> },
    ElementAttributes { attributes: HashMap<String, String> },
    ElementInlineContent { content: String },
    ElementChildren { children: Vec<HtmlToken> },
    EmptyBlockLine,
    BlockLine { content: String },
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
        let html_tokens = Self::lex_html_document(pairs)?;
        let html_body = Self::generate_html_body(&html_tokens, false)?;

        let mut warnings: Vec<railwind::warning::Warning> = Vec::new();
        let generated_css = railwind::parse_to_string(
            railwind::Source::String(html_body.clone(), railwind::CollectionOptions::Html),
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
                * {{ margin: 0; }}
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
            html_body,
        ]);

        Ok(html)
    }

    fn generate_html_body(tokens: &Vec<HtmlToken>, children: bool) -> Result<String> {
        let mut html = String::new();
        let mut element_name = "";
        let mut element_unclosed = false;
        let mut following_block_line = false;

        for token in tokens {
            match token {
                HtmlToken::ElementName { name } => {
                    html.push_str(&format!("<{}", name));

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
                    html.push_str(&Self::generate_html_body(children, true)?);
                    html.push_str(&format!("</{}>", element_name));

                    element_unclosed = false;
                }
                HtmlToken::BlockLine { content } => {
                    if following_block_line {
                        html.push_str("<br>");
                    }
                    html.push_str(content);

                    following_block_line = true;
                }
                HtmlToken::EmptyBlockLine => html.push_str("<br>"),
                // _ => return Err(anyhow!(format!("Unexpected html token: {:?}", token))),
            }
        }

        if element_unclosed && !children {
            html.push('>');
        }

        Ok(html)
    }

    fn lex_html_document(mut pairs: Pairs<Rule>) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();

        for pair in pairs.next().unwrap().into_inner() {
            match pair.as_rule() {
                Rule::element => html.extend(Self::lex_html_element(pair)?),
                Rule::EOI => {}
                Rule::declarations => {}
                _ => return Err(anyhow!(format!("Unexpected document rule: {:?}", pair))),
            }
        }

        Ok(html)
    }

    fn lex_html_children(pairs: Pairs<Rule>) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::block => {
                    for class_pair in pair.into_inner() {
                        match class_pair.as_rule() {
                            Rule::block_content => {
                                html.push(HtmlToken::BlockLine {
                                    content: class_pair.as_span().as_str().to_string(),
                                });
                            }
                            Rule::block_newline => {
                                html.push(HtmlToken::EmptyBlockLine);
                            }
                            Rule::element => {
                                html.extend(Self::lex_html_element(class_pair)?);
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
                    html.extend(Self::lex_html_element(pair)?);
                }
                _ => return Err(anyhow!(format!("Unexpected children rule: {:?}", pair))),
            }
        }

        Ok(html)
    }

    fn lex_html_element(element_pair: Pair<Rule>) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();

        for pair in element_pair.into_inner() {
            match pair.as_rule() {
                Rule::element_name => {
                    html.push(HtmlToken::ElementName {
                        name: pair.as_span().as_str().to_string(),
                    });
                }
                Rule::element_classes => {
                    let mut classes: Vec<String> = Vec::new();

                    for class_pair in pair.into_inner() {
                        if class_pair.as_rule() != Rule::element_class_name {
                            return Err(anyhow!("Unexpected element_classes rule"));
                        }

                        classes.push(class_pair.as_span().as_str().to_string());
                    }

                    html.push(HtmlToken::ElementClasses { classes });
                }
                Rule::element_attributes => {
                    let mut attributes: HashMap<String, String> = HashMap::new();

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
                Rule::element_content => {
                    html.push(HtmlToken::ElementInlineContent {
                        content: pair.as_span().as_str().to_string(),
                    });
                }
                Rule::children => {
                    html.push(HtmlToken::ElementChildren {
                        children: Self::lex_html_children(pair.into_inner())?,
                    });
                }
                _ => return Err(anyhow!(format!("Unexpected element rule: {:?}", pair))),
            }
        }

        Ok(html)
    }
}
