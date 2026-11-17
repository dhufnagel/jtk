//! B007: string literal compared with `==` or `!=` (Checkstyle StringLiteralEquality, PMD
//! UseEqualsToCompareStrings): compares identity, not content.
//!
//! Unsafe fix: `"a".equals(s)` (`!"a".equals(s)` for `!=`), with the literal (or the concatenation with one) first so
//! that a null operand gives `false` as before. It changes the result where identity was meant (interned strings).

use tree_sitter::Node;

use crate::lint::syntax::{is_comment, named_children, operator, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(binary: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let operator_text = operator(binary, text);
    if !matches!(operator_text, "==" | "!=") {
        return;
    }
    let (Some(left), Some(right)) = (binary.child_by_field_name("left"), binary.child_by_field_name("right")) else {
        return;
    };
    if !is_string(&left, text) && !is_string(&right, text) {
        return;
    }
    let operator = binary.child_by_field_name("operator").unwrap_or(*binary);
    let message = "String literal compared with == or != instead of equals()";
    let diagnostic = Diagnostic::new(Rule::StringLiteralEquality, operator.start_byte(), message);
    // Comments between the operands would be lost.
    if named_children(binary).iter().any(is_comment) {
        diagnostics.push(diagnostic);
        return;
    }
    // A plain literal is the best receiver, then a concatenation with one.
    let (receiver, argument) =
        if left.kind() == "string_literal" || (right.kind() != "string_literal" && is_string(&left, text)) {
            (left, right)
        } else {
            (right, left)
        };
    let receiver_code = source(&receiver, text);
    let receiver_code = if matches!(receiver.kind(), "string_literal" | "parenthesized_expression") {
        receiver_code.to_string()
    } else {
        format!("({receiver_code})")
    };
    let negation = if operator_text == "!=" { "!" } else { "" };
    let replacement = format!("{negation}{receiver_code}.equals({})", source(&argument, text));
    diagnostics.push(diagnostic.with_unsafe_fix(vec![Edit::new(binary.byte_range(), replacement)]));
}

/// A string literal, or a concatenation with one.
fn is_string(operand: &Node, text: &str) -> bool {
    match operand.kind() {
        "string_literal" => true,
        "parenthesized_expression" => named_children(operand).iter().any(|o| is_string(o, text)),
        "binary_expression" if operator(operand, text) == "+" => {
            let left = operand.child_by_field_name("left");
            let right = operand.child_by_field_name("right");
            [left, right].iter().flatten().any(|o| is_string(o, text))
        }
        _ => false,
    }
}
