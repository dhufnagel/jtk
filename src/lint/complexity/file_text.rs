//! Checks of the file text, which Checkstyle runs outside the syntax tree:
//!
//! - C007: a file with too many lines (Checkstyle FileLength), reported on the first line,
//! - C020: a line that is too long (Checkstyle LineLength), with tabs expanded to the next multiple of 8 and
//!   `package` and `import` lines ignored (Checkstyle's default `ignorePattern`). The default limit is Checkstyle's
//!   80 columns, so that the results compare with Checkstyle; formatted code (palantir-java-format) is up to 120
//!   columns wide, which `jtk.toml` can set.
//!
//! As in Checkstyle, lines end at `\n`, `\r\n` or `\r`, and a line terminator at the end of the file does not start
//! another line.

use crate::ast::Tree;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

const TAB_WIDTH: usize = 8;

pub fn check_file_length(_unit: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let max = context.options.file_length.max as usize;
    let lines = lines(context.text).count();
    if lines > max {
        let message = format!("File length is {lines} lines (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::FileLength, 0, message));
    }
}

pub fn check_line_length(_unit: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let max = context.options.line_length.max as usize;
    for (start, line) in lines(context.text) {
        let is_short = line.len() <= max && !line.contains('\t');
        if is_short || line.starts_with("package ") || line.starts_with("import ") {
            continue;
        }
        let length = expanded_length(line);
        if length > max {
            let message = format!("Line is longer than {max} characters (found {length}).");
            diagnostics.push(Diagnostic::new(Rule::LineLength, start, message));
        }
    }
}

/// The lines of the text without terminators, with their start offsets.
fn lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let bytes = text.as_bytes();
    let mut start = 0;
    std::iter::from_fn(move || {
        if start >= bytes.len() {
            return None;
        }
        let end = bytes[start..].iter().position(|&b| b == b'\n' || b == b'\r').map_or(bytes.len(), |i| start + i);
        let line = (start, &text[start..end]);
        start = match bytes.get(end) {
            Some(b'\r') if bytes.get(end + 1) == Some(&b'\n') => end + 2,
            Some(_) => end + 1,
            None => end,
        };
        Some(line)
    })
}

/// The number of columns of the line, in code points with tabs expanded.
fn expanded_length(line: &str) -> usize {
    line.chars().fold(0, |length, c| if c == '\t' { (length / TAB_WIDTH + 1) * TAB_WIDTH } else { length + 1 })
}
