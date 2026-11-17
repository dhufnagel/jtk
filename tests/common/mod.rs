//! Helpers shared by the linter tests.

#![allow(dead_code)]

use std::path::Path;

use jtk::lint::{Selection, fix, fix_with, lint, lint_file};
use jtk::newlines::line_column;

/// The rules whose code starts with one of the comma-separated prefixes.
fn selection(code: &str) -> Selection {
    Selection::new(code.split(',').map(str::to_string).collect(), vec![]).unwrap()
}

/// The violations of the rules whose code starts with `code`, as `line:column CODE`.
pub fn violations(code: &str, source: &str) -> Vec<String> {
    let diagnostics = lint(source, &selection(code)).unwrap();
    diagnostics
        .iter()
        .map(|d| {
            let (line, column) = line_column(source, d.offset);
            format!("{line}:{column} {}", d.rule.code())
        })
        .collect()
}

/// The violations of the rules whose code starts with `code` in a file at `path`, as `line:column CODE`.
pub fn file_violations(code: &str, path: &str, source: &str) -> Vec<String> {
    let diagnostics = lint_file(source, Some(Path::new(path)), &selection(code).into()).unwrap();
    diagnostics
        .iter()
        .map(|d| {
            let (line, column) = line_column(source, d.offset);
            format!("{line}:{column} {}", d.rule.code())
        })
        .collect()
}

/// The lines with violations of the rules whose code starts with `code`.
pub fn lines_of(code: &str, source: &str) -> Vec<usize> {
    lint(source, &selection(code)).unwrap().iter().map(|d| line_column(source, d.offset).0).collect()
}

/// The messages of the violations of the rules whose code starts with `code`.
pub fn messages(code: &str, source: &str) -> Vec<String> {
    lint(source, &selection(code)).unwrap().into_iter().map(|d| d.message).collect()
}

/// The source after applying the fixes of the rules whose code starts with `code`.
pub fn fixed(code: &str, source: &str) -> String {
    fix(source, &selection(code)).unwrap()
}

/// The source after applying the safe and unsafe fixes of the rules whose code starts with `code`.
pub fn unsafe_fixed(code: &str, source: &str) -> String {
    fix_with(source, &selection(code).into(), true).unwrap()
}
