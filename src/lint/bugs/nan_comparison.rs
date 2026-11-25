//! B078: a comparison with `Double.NaN` or `Float.NaN`, as in `x == Double.NaN` (Error Prone EqualsNaN, PMD
//! ComparisonWithNaN, IntelliJ ComparisonToNaN): NaN is not equal to anything, itself included, so `==` is always
//! false, `!=` always true, and `<`, `<=`, `>` and `>=` always false.
//!
//! Fix for `==` and `!=`: `Double.isNaN(x)` and `!Double.isNaN(x)` (`Float.isNaN` for `Float.NaN`). It is marked
//! safe like in the catalogue: the old comparison had a constant result, so it never did what it says, and the
//! operand is still evaluated once. The ordering comparisons are reported without a fix, as their intent is unclear.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Binary { op, lhs, rhs } = &tree.kind else { return };
    if !matches!(op.as_str(), "==" | "!=" | "<" | ">" | "<=" | ">=") {
        return;
    }
    let (operand, class) = match (nan_class(lhs, context), nan_class(rhs, context)) {
        (None, Some(class)) => (lhs, class),
        (Some(class), None) => (rhs, class),
        _ => return,
    };
    let diagnostic =
        Diagnostic::new(Rule::NanComparison, tree.start, format!("Comparison with NaN is always {}", op == "!="));
    let fixable = matches!(op.as_str(), "==" | "!=") && !context.tokens().has_comment(tree.start..tree.end);
    diagnostics.push(if fixable {
        let negation = if op == "!=" { "!" } else { "" };
        let replacement = format!("{negation}{class}.isNaN({})", context.source(operand));
        diagnostic.with_fix(vec![Edit::new(tree.start..tree.end, replacement)])
    } else {
        diagnostic
    });
}

/// `Double` or `Float` if the expression is their `NaN` constant.
fn nan_class(expression: &Tree, context: &Context) -> Option<&'static str> {
    let K::MemberSelect { expr, name } = &expression.kind else { return None };
    if name != "NaN" {
        return None;
    }
    match context.source(expr) {
        "Double" | "java.lang.Double" => Some("Double"),
        "Float" | "java.lang.Float" => Some("Float"),
        _ => None,
    }
}
