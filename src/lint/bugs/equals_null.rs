//! B009: `x.equals(null)` (PMD EqualsNull): is always false, or throws if `x` is null.
//!
//! Unsafe fix: `x == null` (Eclipse's quick fix). It changes the behaviour where `x` is null (no exception) or its
//! `equals` breaks the contract.

use tree_sitter::Node;

use crate::lint::syntax::{has_comment, named_children, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(invocation: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let is_equals = invocation.child_by_field_name("name").is_some_and(|n| source(&n, text) == "equals");
    let arguments = invocation.child_by_field_name("arguments").map(|a| named_children(&a)).unwrap_or_default();
    let is_null_argument = matches!(arguments.as_slice(), [argument] if argument.kind() == "null_literal");
    let Some(object) = invocation.child_by_field_name("object") else { return };
    if !is_equals || !is_null_argument {
        return;
    }
    let diagnostic = Diagnostic::new(Rule::EqualsNull, invocation.start_byte(), "equals(null) instead of == null");
    let parent = invocation.parent();
    // A comparison is no statement.
    let fixable = object.kind() != "super"
        && !has_comment(invocation)
        && invocation.child_by_field_name("type_arguments").is_none()
        && parent.is_some_and(|p| p.kind() != "expression_statement");
    diagnostics.push(if fixable {
        // The call binds tighter than `==`: keep the comparison together with parentheses unless the context is
        // known not to need them.
        let comparison = format!("{} == null", source(&object, text));
        let needs_parentheses = parent.is_none_or(|p| !is_low_precedence_context(&p, invocation, text));
        let replacement = if needs_parentheses { format!("({comparison})") } else { comparison };
        diagnostic.with_unsafe_fix(vec![Edit::new(invocation.byte_range(), replacement)])
    } else {
        diagnostic
    });
}

/// Whether an expression can be replaced by a comparison without parentheses in this parent: where it is a whole
/// expression, or an operand of `&&`, `||` or `?:` (conditional operators bind less tightly than `==`).
fn is_low_precedence_context(parent: &Node, child: &Node, text: &str) -> bool {
    match parent.kind() {
        "parenthesized_expression"
        | "argument_list"
        | "variable_declarator"
        | "return_statement"
        | "lambda_expression"
        | "yield_statement"
        | "assert_statement" => true,
        "assignment_expression" => parent.child_by_field_name("right") == Some(*child),
        "binary_expression" => {
            matches!(parent.child_by_field_name("operator").map(|o| source(&o, text)), Some("&&" | "||"))
        }
        "ternary_expression" => parent.child_by_field_name("condition") == Some(*child),
        _ => false,
    }
}
