//! S003: boolean literal in a comparison, logical operator or conditional (Checkstyle SimplifyBooleanExpression, PMD
//! SimplifyBooleanExpressions, SimplifiedTernary): `b == true`, `!false`, `c ? true : false`.
//!
//! Fixed when no operand with possible side effects is dropped: `b == true` is `b`, but `f() || true` stays.

use tree_sitter::Node;

use crate::lint::syntax::{is_boolean_literal, negation, operator, source, strip_parentheses};
use crate::lint::{Diagnostic, Edit, Rule};

const MESSAGE: &str = "Boolean expression can be simplified";

pub fn check(expression: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (position, simplified) = match expression.kind() {
        "binary_expression" => match binary(expression, text) {
            Some(found) => found,
            None => return,
        },
        "unary_expression" => {
            let Some(operand) = expression.child_by_field_name("operand") else { return };
            let operand = strip_parentheses(&operand);
            if operator(expression, text) != "!" || !is_boolean_literal(&operand) {
                return;
            }
            let negated = if operand.kind() == "true" { "false" } else { "true" };
            (expression.start_byte(), Some(negated.to_string()))
        }
        "ternary_expression" => match ternary(expression, text) {
            Some(found) => found,
            None => return,
        },
        _ => return,
    };
    let diagnostic = Diagnostic::new(Rule::SimplifyBooleanExpression, position, MESSAGE);
    let fix = simplified.map(|s| Edit::new(expression.byte_range(), s));
    diagnostics.push(diagnostic.with_fix(fix.into_iter().collect()));
}

/// The position of the operator and the simplified expression, if it can be simplified safely.
fn binary(expression: &Node, text: &str) -> Option<(usize, Option<String>)> {
    let op = operator(expression, text);
    if !matches!(op, "==" | "!=" | "&&" | "||") {
        return None;
    }
    // The other operand keeps its parentheses: `(a || b) == true` in `c && ...` must stay `c && (a || b)`.
    let left = expression.child_by_field_name("left")?;
    let right = expression.child_by_field_name("right")?;
    let (literal, other, literal_first) = if is_boolean_literal(&strip_parentheses(&left)) {
        (strip_parentheses(&left), right, true)
    } else if is_boolean_literal(&strip_parentheses(&right)) {
        (strip_parentheses(&right), left, false)
    } else {
        return None;
    };
    let value = literal.kind() == "true";
    let other_code = || source(&other, text).to_string();
    let simplified = match (op, value) {
        ("==", true) | ("!=", false) => Some(other_code()),
        ("==", false) | ("!=", true) => Some(negation(&other, text)),
        ("&&", true) | ("||", false) => Some(other_code()),
        // `false && x` and `true || x` never evaluate x; `x && false` and `x || true` do.
        ("&&", false) | ("||", true) if literal_first => Some(source(&literal, text).to_string()),
        _ => None,
    };
    let operator_start = expression.child_by_field_name("operator").map_or(expression.start_byte(), |o| o.start_byte());
    Some((operator_start, simplified))
}

/// `true ? a : b`, or `c ? true : false` with literals in both branches.
fn ternary(expression: &Node, text: &str) -> Option<(usize, Option<String>)> {
    let condition = strip_parentheses(&expression.child_by_field_name("condition")?);
    let consequence = strip_parentheses(&expression.child_by_field_name("consequence")?);
    let alternative = strip_parentheses(&expression.child_by_field_name("alternative")?);
    let question = expression.children(&mut expression.walk()).find(|c| c.kind() == "?")?.start_byte();
    if is_boolean_literal(&condition) {
        let chosen = if condition.kind() == "true" { consequence } else { alternative };
        return Some((question, Some(source(&chosen, text).to_string())));
    }
    if !is_boolean_literal(&consequence) || !is_boolean_literal(&alternative) {
        return None;
    }
    let simplified = match (consequence.kind(), alternative.kind()) {
        ("true", "false") => Some(source(&condition, text).to_string()),
        ("false", "true") => Some(negation(&condition, text)),
        _ => None,
    };
    Some((question, simplified))
}
