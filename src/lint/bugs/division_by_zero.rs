//! B096: a division or remainder by a constant zero, as in `x / 0` or `x %= 0` (IntelliJ DivideByZero, javac's
//! `divzero` lint): with integers it throws an `ArithmeticException`, with floating point numbers it gives an
//! infinity or NaN.
//!
//! No fix. Like javac, only an integer zero counts as the divisor (an integer literal or a constant expression of
//! them, such as `(1 - 1)`); a floating point zero (`0.0 / 0.0`) is the usual way to write NaN or infinity in
//! constants. A floating point literal as the dividend (`1.0 / 0`) is left out for the same reason.

use super::shared_arithmetic::constant;
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (K::Binary { op, lhs, rhs } | K::Assign { op, lhs, rhs }) = &tree.kind else { return };
    if !matches!(op.as_str(), "/" | "%" | "/=" | "%=") {
        return;
    }
    let is_zero = constant(rhs, context.text).is_some_and(|v| v.wide() == 0);
    let is_floating_dividend = is_floating_literal(lhs);
    if is_zero && !is_floating_dividend {
        let message = if op.starts_with('/') { "Division by zero" } else { "Remainder by zero" };
        diagnostics.push(Diagnostic::new(Rule::DivisionByZero, tree.start, message));
    }
}

/// Whether the expression is a floating point literal, maybe negated.
fn is_floating_literal(expression: &Tree) -> bool {
    match &strip_parens(expression).kind {
        K::Literal(kind) => matches!(kind, LitKind::Float | LitKind::Double),
        K::Unary { op, expr, .. } if op == "-" || op == "+" => is_floating_literal(expr),
        _ => false,
    }
}
