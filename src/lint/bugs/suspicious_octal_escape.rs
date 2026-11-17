//! B018: an octal escape in a string literal followed by a digit that looks like a part of it, as in `"\128"`
//! (`\12` and `8`) or `"\0123"` (`\012` and `3`) (PMD SuspiciousOctalEscape).
//!
//! Like PMD, only string literals and text blocks are checked, not character literals, and each suspicious escape is
//! reported.

use tree_sitter::Node;

use crate::lint::syntax::source;
use crate::lint::{Diagnostic, Rule};

pub fn check(literal: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let code = source(literal, text).as_bytes();
    if code.len() < 2 || !code.contains(&b'\\') {
        return;
    }
    // Without the quotes, as PMD does (a text block keeps two of its three quotes on each side).
    let content = &code[1..code.len() - 1];
    let mut offset = 0;
    while let Some(slash) = content[offset..].iter().position(|&c| c == b'\\').map(|i| offset + i) {
        if slash + 1 >= content.len() {
            break;
        }
        let escape = &content[slash + 1..];
        offset = slash + 1;
        if escape[0] == b'\\' {
            offset += 1;
        } else if let Some(split) = suspicious_split(escape) {
            let (octal, digit) = escape.split_at(split);
            let message = format!(
                "Suspicious decimal characters following octal escape in string literal: \\{} + {}",
                String::from_utf8_lossy(octal),
                char::from(digit[0])
            );
            diagnostics.push(Diagnostic::new(Rule::SuspiciousOctalEscape, literal.start_byte(), message));
        }
    }
}

/// The length of the octal escape at the start of `escape` (after the backslash) if a decimal digit follows it that
/// could be taken for a part of it.
fn suspicious_split(escape: &[u8]) -> Option<usize> {
    let is_octal = |i: usize| escape.get(i).is_some_and(|c| (b'0'..=b'7').contains(c));
    let is_decimal = |i: usize| escape.get(i).is_some_and(u8::is_ascii_digit);
    if !is_octal(0) {
        return None;
    }
    if !is_octal(1) {
        return is_decimal(1).then_some(1);
    }
    if !is_octal(2) {
        return is_decimal(2).then_some(2);
    }
    // Three octal digits: one escape if it starts with 0 to 3, otherwise two digits and a digit.
    if escape[0] > b'3' { Some(2) } else { is_decimal(3).then_some(3) }
}
