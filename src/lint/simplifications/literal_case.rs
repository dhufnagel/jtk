//! S015: the letter case in numeric literals (Checkstyle UpperEll, HexLiteralCase and
//! NumericalPrefixesInfixesSuffixesCharacterCase, Error Prone LongLiteralLowerCaseSuffix): an uppercase `L` suffix
//! (`1l` looks like `11`), uppercase hexadecimal digits, and lowercase prefixes (`0x`, `0b`), exponents (`e`, `p`)
//! and `f`/`d` suffixes.
//!
//! The fix rewrites the literal in that case.

use tree_sitter::Node;

use crate::lint::syntax::source;
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(literal: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let code = source(literal, text);
    let is_floating = literal.kind().ends_with("floating_point_literal");
    let mut messages = Vec::new();
    if !is_floating && code.ends_with('l') {
        messages.push("Should use uppercase 'L'");
    }
    if code.starts_with("0x") || code.starts_with("0X") {
        // In a floating point literal, the last letter is the suffix, not a digit.
        let digits = if is_floating { &code[..code.len() - 1] } else { code };
        if digits.bytes().any(|c| c.is_ascii_hexdigit() && c.is_ascii_lowercase()) {
            messages.push("Should use uppercase hexadecimal letters");
        }
    }
    let is_hex = code.starts_with("0x");
    if code.starts_with("0X") || code.starts_with("0B") {
        messages.push("Numerical prefix should be in lowercase");
    } else if (!is_hex && code.contains('E')) || code.contains('P') {
        messages.push("Numerical infix should be in lowercase");
    } else if (code.ends_with('F') || code.ends_with('D')) && (!is_hex || code.contains('p')) {
        messages.push("Numerical suffix should be in lowercase");
    }
    if messages.is_empty() {
        return;
    }
    let fix = Edit::new(literal.byte_range(), canonical(code, is_floating));
    for message in messages {
        diagnostics.push(Diagnostic::new(Rule::LiteralCase, literal.start_byte(), message).with_fix(vec![fix.clone()]));
    }
}

/// The literal with lowercase prefix, exponent and `f`/`d` suffix, uppercase hexadecimal digits and `L` suffix.
fn canonical(code: &str, is_floating: bool) -> String {
    let lower = code.to_ascii_lowercase();
    let Some(hex) = lower.strip_prefix("0x") else {
        return match lower.strip_suffix('l') {
            Some(number) => format!("{number}L"),
            None => lower,
        };
    };
    if is_floating {
        // Digits, then `p`, the exponent and an optional suffix: only the digits are uppercase.
        let (digits, exponent) = hex.split_once('p').unwrap_or((hex, ""));
        format!("0x{}p{exponent}", digits.to_ascii_uppercase())
    } else {
        match hex.strip_suffix('l') {
            Some(digits) => format!("0x{}L", digits.to_ascii_uppercase()),
            None => format!("0x{}", hex.to_ascii_uppercase()),
        }
    }
}
