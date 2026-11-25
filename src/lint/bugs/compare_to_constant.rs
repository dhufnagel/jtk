//! B054: the result of `compareTo` or `compare` compared with a constant other than 0, as in `a.compareTo(b) == 1`
//! (Error Prone CompareToZero, IntelliJ ComparatorResultComparison): the contract only fixes the sign of the result,
//! and many implementations (`String.compareTo`) return other values.
//!
//! Fix: compare the sign with 0 (`a.compareTo(b) > 0`), offered for comparisons with 1 and -1 as Error Prone does.
//! It is marked safe like in the catalogue: it keeps the behaviour wherever the old comparison worked, that is for
//! results -1, 0 and 1, and corrects it for other results. Comparisons with other constants are reported without a
//! fix.
//!
//! Without types, the calls are recognised by their shape: `compareTo` with one argument, `compare` with two on the
//! wrapper classes of `java.lang` or on a variable declared in the file as a `Comparator`, and
//! `Objects.compare(a, b, c)`. Unlike Error Prone, arithmetic on the result and `switch` on it are not reported, and
//! only literals are recognised as constants.

use super::shared_arithmetic::{Value, integer_literal, is_comparison, method_name, mirrored, variable_type};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

const WRAPPERS: &[&str] = &["Boolean", "Byte", "Character", "Double", "Float", "Integer", "Long", "Short"];

pub fn check<'t>(tree: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let K::Binary { op, lhs, rhs } = &tree.kind else { return };
    if !is_comparison(op) {
        return;
    }
    let text = context.text;
    let (call, op, constant) = if is_comparison_call(lhs, tree, context) {
        (lhs, op.as_str(), rhs)
    } else if is_comparison_call(rhs, tree, context) {
        (rhs, mirrored(op), lhs)
    } else {
        return;
    };
    let Some(constant) = integer_literal(constant, text).map(Value::wide).filter(|&c| c != 0) else { return };
    let replacement = match (op, constant) {
        ("==", 1) | (">=", 1) => Some(">"),
        ("==", -1) | ("<=", -1) => Some("<"),
        ("!=", 1) | ("<", 1) => Some("<="),
        ("!=", -1) | (">", -1) => Some(">="),
        _ => None,
    };
    let message = "compareTo result compared with a constant other than 0";
    let diagnostic = Diagnostic::new(Rule::CompareToConstant, tree.start, message);
    diagnostics.push(match replacement {
        Some(replacement) if !context.tokens().has_comment(tree.start..tree.end) => {
            let fixed = format!("{} {replacement} 0", context.source(call));
            diagnostic.with_fix(vec![Edit::new(tree.start..tree.end, fixed)])
        }
        _ => diagnostic,
    });
}

/// Whether `expression` is a call of a method whose result only has a meaningful sign.
fn is_comparison_call<'t>(expression: &'t Tree, tree: &'t Tree, context: &Context<'t>) -> bool {
    // A call in parentheses is fixed with them.
    let K::MethodInvocation { select, args, .. } = &strip_parens(expression).kind else { return false };
    let receiver = match &select.kind {
        K::MemberSelect { expr, .. } => Some(&**expr),
        _ => None,
    };
    let receiver_source =
        receiver.map(|r| context.source(r).trim_start_matches("java.lang.").trim_start_matches("java.util."));
    match (method_name(select), args.len()) {
        (Some("compareTo"), 1) => true,
        (Some("compare"), 2) => {
            receiver_source.is_some_and(|r| WRAPPERS.contains(&r))
                || receiver.and_then(|r| variable_type(context, tree, r)).is_some_and(|t| t == "Comparator")
        }
        (Some("compare"), 3) => receiver_source == Some("Objects"),
        _ => false,
    }
}
