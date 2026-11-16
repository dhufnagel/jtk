use std::io::{Read, Write};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

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
    // Files are formatted in parallel; output and errors are reported in the order the files were given.
    let results = format_files(&files, &options, in_place);
    let mut status = ExitCode::SUCCESS;
    let mut stdout = std::io::stdout().lock();
    for (file, result) in files.iter().zip(results) {
        match result {
            Ok(Some(out)) => {
                let _ = stdout.write_all(out.as_bytes());
            }
            Ok(None) => {}
            Err(e) => {
                let _ = stdout.flush();
                eprintln!("{file}: {e}");
                status = ExitCode::FAILURE;
            }
        }
    }
    status
}

/// Formats the files on all available cores. With `in_place`, results are written back and `Ok(None)` is returned;
/// otherwise the formatted text is returned.
fn format_files(files: &[String], options: &FormatOptions, in_place: bool) -> Vec<Result<Option<String>, String>> {
    let format_one = |file: &String| -> Result<Option<String>, String> {
        let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        let out = format(&text, options).map_err(|e| format!("error: {e}"))?;
        if !in_place {
            return Ok(Some(out));
        }
        if out != text {
            std::fs::write(file, out).map_err(|e| e.to_string())?;
        }
        Ok(None)
    };
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).min(files.len());
    if workers <= 1 {
        return files.iter().map(format_one).collect();
    }
    let next = AtomicUsize::new(0);
    let mut results: Vec<(usize, Result<Option<String>, String>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(file) = files.get(i) else { break };
                        done.push((i, format_one(file)));
                    }
                    done
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("formatter worker panicked")).collect()
    });
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, r)| r).collect()
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
