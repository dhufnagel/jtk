//! Parses every `.java` file below a directory with the hand-written parser and with tree-sitter (converted), and
//! checks that both produce the same tree. Files the hand-written parser cannot parse are formatted through
//! tree-sitter, so they are only counted; files only tree-sitter rejects would change behaviour and are listed.
//!
//! Usage: `cargo run --release --example parser_check -- <dir>`

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
    let dir = std::env::args().nth(1).expect("usage: parser_check <dir>");
    let mut files = Vec::new();
    collect(Path::new(&dir), &mut files);
    files.sort();
    // Deeply nested input needs a large stack, as when formatting.
    let code = std::thread::Builder::new().stack_size(1 << 28).spawn(move || check(&files)).unwrap().join().unwrap();
    std::process::exit(code);
}

fn check(files: &[PathBuf]) -> i32 {
    let (mut same, mut own_failed, mut problems) = (0, 0, 0);
    let (mut tree_sitter_time, mut own_time) = (Duration::ZERO, Duration::ZERO);
    for path in files {
        let Ok(text) = std::fs::read_to_string(path) else { continue };
        let start = Instant::now();
        let converted = jtk::parse::parse(&text)
            .and_then(|tree| jtk::ast::Converter { src: &text }.compilation_unit(&tree.root_node()).ok());
        tree_sitter_time += start.elapsed();
        let start = Instant::now();
        let own = jtk::parser::parse(&text).ok();
        own_time += start.elapsed();
        match (converted, own) {
            (Some(a), Some(b)) => {
                let (a, b) = (format!("{a:?}"), format!("{b:?}"));
                if a == b {
                    same += 1;
                } else {
                    problems += 1;
                    let i = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(0);
                    let lo = i.saturating_sub(200);
                    println!(
                        "{}: different trees\n  tree-sitter: ...{}\n  parser:      ...{}",
                        path.display(),
                        &a[lo..(i + 200).min(a.len())],
                        &b[lo..(i + 200).min(b.len())]
                    );
                }
            }
            (Some(_), None) => own_failed += 1,
            (None, Some(_)) => {
                problems += 1;
                println!("{}: only tree-sitter rejects it", path.display());
            }
            (None, None) => {}
        }
    }
    println!(
        "{} files: {same} same, {own_failed} left to tree-sitter, {problems} problems; tree-sitter {:.2?}, parser {:.2?}",
        files.len(),
        tree_sitter_time,
        own_time
    );
    i32::from(problems > 0)
}
