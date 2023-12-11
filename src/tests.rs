use crate::parser::{DocumentParser, Rule};
use pest::Parser;
use std::ffi::OsStr;
use std::{env, fs};

#[test]
fn build_documentation() {
    let cwd = env::current_dir().unwrap();
    let docs = cwd.join("docs");

    env::set_current_dir(&cwd.join(&docs)).unwrap();

    for entry in fs::read_dir(docs).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().unwrap_or(OsStr::new("")) != "twml" {
            continue;
        }

        let document = fs::read_to_string(&path).unwrap();
        let pairs = DocumentParser::parse(Rule::document, &document).unwrap();
        let declarations = DocumentParser::get_declarations(pairs.clone()).unwrap();
        let _ = DocumentParser::generate_html(&declarations, pairs).unwrap();
    }
}
