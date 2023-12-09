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
    pub js: Vec<String>,
    pub css: Vec<String>,
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
        let mut js: Vec<String> = Vec::new();
        let mut css: Vec<String> = Vec::new();
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
            page_width_mm,
            page_height_mm,
        })
    }

    pub fn generate_html(declarations: &Declarations, pairs: Pairs<Rule>) -> Result<String> {
        let html_tokens = Self::lex_html_document(pairs)?;
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
                  }}
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

    fn lex_html_document(mut pairs: Pairs<Rule>) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();

        for pair in pairs.next().unwrap().into_inner() {
            match pair.as_rule() {
                Rule::block => html.extend(Self::lex_html_block(pair.into_inner())?),
                Rule::declaration => {}
                Rule::EOI => {}
                _ => return Err(anyhow!(format!("Unexpected document rule: {:?}", pair))),
            }
        }

        Ok(html)
    }

    fn lex_html_block(pairs: Pairs<Rule>) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::block_element => html.extend(Self::lex_html_block_element(pair)?),
                Rule::block_content_line => {
                    html.push(HtmlToken::BlockLine {
                        content: pair.as_span().as_str().to_string(),
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

    fn lex_html_block_children(pairs: Pairs<Rule>) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();

        for pair in pairs {
            match pair.as_rule() {
                Rule::block => html.extend(Self::lex_html_block(pair.into_inner())?),
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

    fn lex_html_block_element(element_pair: Pair<Rule>) -> Result<Vec<HtmlToken>> {
        let mut html: Vec<HtmlToken> = Vec::new();

        for pair in element_pair.into_inner() {
            match pair.as_rule() {
                Rule::block_element_name => {
                    html.push(HtmlToken::ElementName {
                        name: pair.as_span().as_str().to_string(),
                    });
                }
                Rule::block_element_classes => {
                    let mut classes: Vec<String> = Vec::new();

                    for class_pair in pair.into_inner() {
                        if class_pair.as_rule() != Rule::block_element_class {
                            return Err(anyhow!("Unexpected element_classes rule"));
                        }

                        classes.push(class_pair.as_span().as_str().to_string());
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
                    html.push(HtmlToken::ElementInlineContent {
                        content: pair.as_span().as_str().to_string(),
                    });
                }
                Rule::block_children => {
                    html.push(HtmlToken::ElementChildren {
                        children: Self::lex_html_block_children(pair.into_inner())?,
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
}
