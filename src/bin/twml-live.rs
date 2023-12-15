use anyhow::{Context, Result};
use notify::Watcher;
use penguin::Server;
use pest::Parser;
use std::env;
use std::fs;
use std::io::Write;
use std::process::exit;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use twml::parser::{DocumentParser, LexerState, Rule};

#[tokio::main]
async fn main() -> Result<()> {
    let arguments: Vec<String> = env::args().collect();

    if arguments.len() != 2 {
        println!("Usage: {} <input.twml>", arguments[0]);
        exit(22);
    }

    let document_path =
        fs::canonicalize(&arguments[1]).context("Received an unexpected input path")?;
    let document_parent_path = &document_path.parent().unwrap().to_path_buf();

    let time = SystemTime::now().duration_since(UNIX_EPOCH)?.subsec_nanos();
    let temporary_dir_path = env::temp_dir().join(format!("twml-live-{}", time));
    let temporary_dir_path_drop_copy = temporary_dir_path.clone();
    fs::create_dir(&temporary_dir_path).context("Failed to create a temporary directory")?;

    let index_path = temporary_dir_path.join("index.html");
    let mut index_file = fs::File::create(&index_path)?;

    let (server, controller) = Server::bind(([127, 0, 0, 1], 8080).into())
        .add_mount("/", &temporary_dir_path)?
        .build()?;

    println!("Server running on: http://127.0.0.1:8080/");

    let mut watcher = notify::recommended_watcher(
        move |result: Result<notify::Event, notify::Error>| match result {
            Ok(event) => {
                if event.kind.is_modify() && event.paths.contains(&document_path) {
                    // Some programs (formatters?) seem to remove a file shortly after modifying
                    // and this can cause race conditions. Therefore we introduce a small delay,
                    // hoping that this will decrease the risk of this race condition from
                    // happening.
                    thread::sleep(Duration::from_millis(200));

                    let document = fs::read_to_string(&document_path)
                        .expect("Failed to read the input document");
                    let pairs_result = DocumentParser::parse(Rule::document, &document);

                    match pairs_result {
                        Ok(pairs) => {
                            let declarations = DocumentParser::get_declarations(pairs.clone())
                                .expect("Failed to parse the declarations");
                            let mut lex_state = LexerState::default();
                            let html =
                                DocumentParser::generate_html(&declarations, &mut lex_state, pairs)
                                    .expect("Failed to generate html code");

                            let _ = index_file.set_len(0);
                            write!(index_file, "{}", html).unwrap();

                            DocumentParser::include_linked_files(
                                &declarations,
                                &temporary_dir_path,
                            )
                            .expect("Failed to include linked files");

                            controller.reload();
                        }
                        Err(error) => {
                            controller.show_message(error.to_string().replace('\n', "<br>"))
                        }
                    }
                }
            }
            Err(error) => println!("File watch error: {:?}", error),
        },
    )?;

    watcher.watch(document_parent_path, notify::RecursiveMode::NonRecursive)?;

    server.await?;

    fs::remove_dir_all(&temporary_dir_path_drop_copy)
        .context("Failed to clean up temporary directory")?;

    Ok(())
}
