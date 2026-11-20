//! C007: a file with too many lines (Checkstyle FileLength, which runs on the file text rather than the syntax
//! tree), reported on the first line.
//!
//! As in Checkstyle, lines end at `\n`, `\r\n` or `\r`, and a line terminator at the end of the file does not start
//! another line.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::Tree;

pub fn check_file_length(_unit: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let max = context.options.file_length.max as usize;
    let lines = lines(context.text).count();
    if lines > max {
        let message = format!("File length is {lines} lines (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::FileLength, 0, message));
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
