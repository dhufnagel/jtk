//! V014: a Unicode character that changes the direction of text, written as is anywhere in the source, comments and
//! string literals included (Error Prone UnicodeDirectionalityCharacters): editors then show the code in an order
//! that differs from what the compiler reads ("Trojan Source", CVE-2021-42574).
//!
//! No fix. Besides Error Prone's embeddings, overrides and isolates (U+202A to U+202E, U+2066 to U+2069), the
//! implicit marks U+200E, U+200F and U+061C count too. The check reads the raw text, so a Unicode escape (a
//! backslash, `u` and the code) is no violation.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::Tree;

pub fn check(_: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    for (offset, c) in context.text.char_indices() {
        if matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}') {
            let message = format!("Unicode directionality character U+{:04X}", c as u32);
            diagnostics.push(Diagnostic::new(Rule::BidiCharacter, offset, message));
        }
    }
}
