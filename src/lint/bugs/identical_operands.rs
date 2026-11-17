//! B011: assignment of a variable to itself, and binary operations with identical operands (Error Prone
//! SelfAssignment, IdentityBinaryExpression): `x = x`, `a == a`, `b && b`. Only variables and field accesses are
//! compared, as other operands may have side effects or be deliberate constants. `x != x` tests for NaN, but
//! `Double.isNaN` says so, so it is reported too.

use tree_sitter::Node;

use crate::lint::syntax::{is_side_effect_free, operator, source, strip_parentheses};
use crate::lint::{Diagnostic, Rule};

const OPERATORS: &[&str] = &["==", "!=", "<", "<=", ">", ">=", "&&", "||", "&", "|", "^"];

pub fn check(expression: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (Some(left), Some(right)) = (expression.child_by_field_name("left"), expression.child_by_field_name("right"))
    else {
        return;
    };
    let message = match (expression.kind(), operator(expression, text)) {
        ("assignment_expression", "=") => "Variable is assigned to itself",
        ("binary_expression", op) if OPERATORS.contains(&op) => "Both operands are identical",
        _ => return,
    };
    let (left, right) = (strip_parentheses(&left), strip_parentheses(&right));
    // Identical literals (`1 == 1`) are constant expressions, usually deliberate.
    let is_variable = matches!(left.kind(), "identifier" | "field_access");
    if is_variable && is_side_effect_free(&left) && normalized(&left, text) == normalized(&right, text) {
        diagnostics.push(Diagnostic::new(Rule::IdenticalOperands, expression.start_byte(), message));
    }
}

fn normalized(expression: &Node, text: &str) -> String {
    source(expression, text).split_whitespace().collect()
}
