//! B090: an oddness check `x % 2 == 1` (IntelliJ BadOddness): the remainder of a negative odd number is -1, so the
//! check is false for it. `x % 2 != 1` is true for it.
//!
//! Unsafe fix: `x % 2 != 0` (and `x % 2 == 0` for `!=`), which is right for every sign. It changes the behaviour for
//! negative odd numbers, which is the point, but code that relies on them being treated as even would change too.
//!
//! Whether `x` can be negative is not known without types and data flow, which is the point of the rule. Only the
//! obviously non-negative forms are skipped: sizes and lengths (`size()`, `length()`, `count()`, `.length`),
//! `Math.abs(..)` and literals.

use super::shared_arithmetic::{constant, is_call};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Binary { op, lhs, rhs } = &tree.kind else { return };
    if op != "==" && op != "!=" {
        return;
    }
    let text = context.text;
    let is_one = |t: &Tree| matches!(t.kind, K::Literal(_)) && constant(t, text).is_some_and(|v| v.wide() == 1);
    let (remainder, one) = match (is_one(lhs), is_one(rhs)) {
        (false, true) => (lhs, rhs),
        (true, false) => (rhs, lhs),
        _ => return,
    };
    let K::Binary { op: remainder_op, lhs: dividend, rhs: divisor } = &strip_parens(remainder).kind else { return };
    let is_two = matches!(divisor.kind, K::Literal(_)) && constant(divisor, text).is_some_and(|v| v.wide() == 2);
    if remainder_op != "%" || !is_two || is_non_negative(dividend) {
        return;
    }
    let message = format!("x % 2 {op} 1 is wrong for negative odd numbers; compare the remainder with 0");
    let diagnostic = Diagnostic::new(Rule::OddnessCheck, tree.start, message);
    let tokens = context.tokens();
    let operator = tokens.in_range(lhs.end..rhs.start).next();
    diagnostics.push(match operator {
        Some(operator) => {
            let flipped = if op == "==" { "!=" } else { "==" };
            let edits = vec![Edit::new(operator, flipped), Edit::new(one.start..one.end, "0")];
            diagnostic.with_unsafe_fix(edits)
        }
        None => diagnostic,
    });
}

/// Whether the expression is obviously never negative.
fn is_non_negative(expression: &Tree) -> bool {
    let expression = strip_parens(expression);
    match &expression.kind {
        K::Literal(_) => true,
        K::MemberSelect { name, .. } => name == "length",
        _ => {
            ["size", "length", "count"].iter().any(|name| is_call(expression, name, 0)) || is_call(expression, "abs", 1)
        }
    }
}
