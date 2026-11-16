//! Formats every `.java` file below a directory and checks that formatting succeeds and is idempotent.
//!
//! Usage: `cargo run --release --example corpus_check -- <dir>`

use std::path::{Path, PathBuf};
use std::time::Instant;

use jtk::{FormatOptions, format};

fn format_source(text: &str) -> Result<String, jtk::FormatError> {
    format(text, &FormatOptions::default())
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|e| e == "java") {
            out.push(path);
        }
    }
}

fn main() {
    let dir = std::env::args().nth(1).expect("usage: corpus_check <dir>");
    let mut files = Vec::new();
    collect(Path::new(&dir), &mut files);
    files.sort();
    let (mut ok, mut errors, mut unstable) = (0, Vec::new(), Vec::new());
    let start = Instant::now();
    for f in &files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        match std::panic::catch_unwind(|| format_source(&text)) {
            Ok(Ok(once)) => match format_source(&once) {
                Ok(twice) if twice == once => ok += 1,
                Ok(twice) => {
                    let line = once.lines().zip(twice.lines()).position(|(a, b)| a != b).unwrap_or(0) + 1;
                    unstable.push(format!("{}:{line}", f.display()));
                }
                Err(e) => unstable.push(format!("{}: second pass error: {e}", f.display())),
            },
            Ok(Err(e)) => errors.push(format!("{}: {e}", f.display())),
            Err(_) => errors.push(format!("{}: panic", f.display())),
        }
    }
    for e in &errors {
        println!("ERROR {e}");
    }
    for u in &unstable {
        println!("UNSTABLE {u}");
    }
    println!(
        "{} files in {:.2?}: {ok} ok, {} errors, {} not idempotent",
        files.len(),
        start.elapsed(),
        errors.len(),
        unstable.len()
    );
}
