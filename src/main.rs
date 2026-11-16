use std::io::{Read, Write};
use std::process::ExitCode;

use palantir_java_format::{FormatOptions, format};

const USAGE: &str = "Usage: palantir-java-format [options] [files...]

Formats Java source files in the Palantir style. Reads stdin if no files are given.

Options:
  -i, --replace                    Write the result back to the files instead of stdout
  --skip-sorting-imports           Do not sort imports
  --skip-removing-unused-imports   Do not remove unused imports
  --skip-reflowing-long-strings    Do not reflow string literals that exceed the column limit
  -h, --help                       Print this help";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut options = FormatOptions::default();
    let mut in_place = false;
    let mut files = Vec::new();
    for arg in &args {
        match arg.as_str() {
            "-i" | "--replace" => in_place = true,
            "--skip-sorting-imports" => options.sort_imports = false,
            "--skip-removing-unused-imports" => options.remove_unused_imports = false,
            "--skip-reflowing-long-strings" => options.reflow_long_strings = false,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--debug-tree" => return debug_tree(),
            a if a.starts_with('-') && a != "-" => {
                eprintln!("unknown option: {a}\n\n{USAGE}");
                return ExitCode::from(2);
            }
            _ => files.push(arg.clone()),
        }
    }
    if files.is_empty() || files == ["-"] {
        let mut input = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut input) {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
        return match format(&input, &options) {
            Ok(out) => {
                let _ = std::io::stdout().write_all(out.as_bytes());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("<stdin>: error: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let mut status = ExitCode::SUCCESS;
    for file in &files {
        let result = std::fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|text| format(&text, &options).map(|out| (text, out)).map_err(|e| e.to_string()));
        match result {
            Ok((text, out)) if in_place => {
                if out != text
                    && let Err(e) = std::fs::write(file, out)
                {
                    eprintln!("{file}: {e}");
                    status = ExitCode::FAILURE;
                }
            }
            Ok((_, out)) => {
                let _ = std::io::stdout().write_all(out.as_bytes());
            }
            Err(e) => {
                eprintln!("{file}: error: {e}");
                status = ExitCode::FAILURE;
            }
        }
    }
    status
}

/// Prints the tree-sitter syntax tree of stdin (for debugging the parser adapter).
fn debug_tree() -> ExitCode {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_java::LANGUAGE.into()).unwrap();
    let tree = parser.parse(&input, None).unwrap();
    println!("{}", tree.root_node().to_sexp());
    ExitCode::SUCCESS
}
