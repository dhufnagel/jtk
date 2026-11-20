//! S003: boolean literal in a comparison, logical operator or conditional (Checkstyle SimplifyBooleanExpression, PMD
//! SimplifyBooleanExpressions, SimplifiedTernary): `b == true`, `!false`, `c ? true : false`.
//!
//! Fixed when no operand with possible side effects is dropped: `b == true` is `b`, but `f() || true` stays.

use crate::lint::tree::{Context, is_boolean_literal, negation, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

const MESSAGE: &str = "Boolean expression can be simplified";

pub fn check(expression: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let found = match &expression.kind {
        K::Binary { op, lhs, rhs } => binary(op, lhs, rhs, context),
        K::Unary { op, expr, .. } if op == "!" => {
            let operand = strip_parens(expr);
            is_boolean_literal(operand).then(|| {
                let negated = if context.source(operand) == "true" { "false" } else { "true" };
                (expression.start, Some(negated.to_string()))
            })
        }
        K::Conditional { cond, then, else_ } => ternary(cond, then, else_, context),
        _ => None,
    };
    let Some((position, simplified)) = found else { return };
    let diagnostic = Diagnostic::new(Rule::SimplifyBooleanExpression, position, MESSAGE);
    let fix = simplified.map(|s| Edit::new(expression.start..expression.end, s));
    diagnostics.push(diagnostic.with_fix(fix.into_iter().collect()));
}

/// The position of the operator and the simplified expression, if it can be simplified safely.
fn binary(op: &str, left: &Tree, right: &Tree, context: &Context) -> Option<(usize, Option<String>)> {
    if !matches!(op, "==" | "!=" | "&&" | "||") {
        return None;
    }
    // The other operand keeps its parentheses: `(a || b) == true` in `c && ...` must stay `c && (a || b)`.
    let (literal, other, literal_first) = if is_boolean_literal(strip_parens(left)) {
        (strip_parens(left), right, true)
    } else if is_boolean_literal(strip_parens(right)) {
        (strip_parens(right), left, false)
    } else {
        return None;
    };
    let value = context.source(literal) == "true";
    let other_code = || context.source(other).to_string();
    let simplified = match (op, value) {
        ("==", true) | ("!=", false) => Some(other_code()),
        ("==", false) | ("!=", true) => Some(negation(other, context.text)),
        ("&&", true) | ("||", false) => Some(other_code()),
        // `false && x` and `true || x` never evaluate x; `x && false` and `x || true` do.
        ("&&", false) | ("||", true) if literal_first => Some(context.source(literal).to_string()),
        _ => None,
    };
    let operator_start = context.tokens().next(left.end).map_or(left.start, |o| o.start);
    Some((operator_start, simplified))
}

/// `true ? a : b`, or `c ? true : false` with literals in both branches.
fn ternary(
    condition: &Tree,
    consequence: &Tree,
    alternative: &Tree,
    context: &Context,
) -> Option<(usize, Option<String>)> {
    let question = context.tokens().next(condition.end)?.start;
    let (condition, consequence, alternative) =
        (strip_parens(condition), strip_parens(consequence), strip_parens(alternative));
    if is_boolean_literal(condition) {
        let chosen = if context.source(condition) == "true" { consequence } else { alternative };
        return Some((question, Some(context.source(chosen).to_string())));
    }
    if !is_boolean_literal(consequence) || !is_boolean_literal(alternative) {
        return None;
    }
    let simplified = match (context.source(consequence), context.source(alternative)) {
        ("true", "false") => Some(context.source(condition).to_string()),
        ("false", "true") => Some(negation(condition, context.text)),
        _ => None,
    };
    Some((question, simplified))
}
