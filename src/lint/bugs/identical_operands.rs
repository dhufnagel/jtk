//! B011: assignment of a variable to itself, and binary operations with identical operands (Error Prone
//! SelfAssignment, IdentityBinaryExpression): `x = x`, `a == a`, `b && b`. Only variables and field accesses are
//! compared, as other operands may have side effects or be deliberate constants. `x != x` tests for NaN, but
//! `Double.isNaN` says so, so it is reported too.

use crate::ast::{K, Tree};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};

const OPERATORS: &[&str] = &["==", "!=", "<", "<=", ">", ">=", "&&", "||", "&", "|", "^"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (message, left, right) = match &tree.kind {
        // An annotation's `k = v` is no assignment.
        K::Assign { op, lhs, rhs }
            if op == "=" && !context.parent().is_some_and(|p| matches!(p.kind, K::Annotation { .. })) =>
        {
            ("Variable is assigned to itself", lhs, rhs)
        }
        K::Binary { op, lhs, rhs } if OPERATORS.contains(&op.as_str()) => ("Both operands are identical", lhs, rhs),
        _ => return,
    };
    let (left, right) = (strip_parens(left), strip_parens(right));
    // Identical literals (`1 == 1`) are constant expressions, usually deliberate.
    let is_variable = match &left.kind {
        K::Ident(name) => name != "this" && name != "super",
        K::MemberSelect { name, .. } => name != "class",
        _ => false,
    };
    if is_variable && is_side_effect_free(left) && normalized(left, context) == normalized(right, context) {
        diagnostics.push(Diagnostic::new(Rule::IdenticalOperands, tree.start, message));
    }
}

/// Whether evaluating the name or field access has no side effects: a field of a name, `this` or a field of one.
fn is_side_effect_free(expression: &Tree) -> bool {
    match &expression.kind {
        K::Ident(name) => name != "super",
        K::MemberSelect { expr, name } => name != "class" && is_side_effect_free(expr),
        _ => false,
    }
}

fn normalized(expression: &Tree, context: &Context) -> String {
    context.source(expression).split_whitespace().collect()
}
