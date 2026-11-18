//! B007: string literal compared with `==` or `!=` (Checkstyle StringLiteralEquality, PMD
//! UseEqualsToCompareStrings): compares identity, not content.
//!
//! Unsafe fix: `"a".equals(s)` (`!"a".equals(s)` for `!=`), with the literal (or the concatenation with one) first so
//! that a null operand gives `false` as before. It changes the result where identity was meant (interned strings).

use crate::ast::{K, LitKind, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Binary { op, lhs: left, rhs: right } = &tree.kind else { return };
    if !matches!(op.as_str(), "==" | "!=") || !is_string(left) && !is_string(right) {
        return;
    }
    let operator = context.tokens().in_range(left.end..right.start).next().map_or(tree.start, |o| o.start);
    let message = "String literal compared with == or != instead of equals()";
    let diagnostic = Diagnostic::new(Rule::StringLiteralEquality, operator, message);
    // Comments between the operands would be lost.
    if context.tokens().has_comment(left.end..right.start) {
        diagnostics.push(diagnostic);
        return;
    }
    // A plain literal is the best receiver, then a concatenation with one.
    let (receiver, argument) =
        if is_literal(left) || (!is_literal(right) && is_string(left)) { (left, right) } else { (right, left) };
    let receiver_code = context.source(receiver);
    let receiver_code = if is_literal(receiver) || matches!(receiver.kind, K::Parens(_)) {
        receiver_code.to_string()
    } else {
        format!("({receiver_code})")
    };
    let negation = if op == "!=" { "!" } else { "" };
    let replacement = format!("{negation}{receiver_code}.equals({})", context.source(argument));
    diagnostics.push(diagnostic.with_unsafe_fix(vec![Edit::new(tree.start..tree.end, replacement)]));
}

fn is_literal(tree: &Tree) -> bool {
    matches!(tree.kind, K::Literal(LitKind::String))
}

/// A string literal, or a concatenation with one.
fn is_string(operand: &Tree) -> bool {
    match &operand.kind {
        K::Literal(LitKind::String) => true,
        K::Parens(inner) => is_string(inner),
        K::Binary { op, lhs, rhs } if op == "+" => is_string(lhs) || is_string(rhs),
        _ => false,
    }
}
