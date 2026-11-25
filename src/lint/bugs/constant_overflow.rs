//! B093: a constant integer expression that overflows, as in `long ms = 365 * 24 * 3600 * 1000;` (Error Prone
//! ConstantOverflow, IntelliJ NumericOverflow): the `int` arithmetic wraps before the result is widened to `long`.
//!
//! Unsafe fix, like Error Prone's: make the leftmost literal a `long` (`365L * 24 * 3600 * 1000`). It is offered
//! only where the expression initialises a variable declared `long` (so the new type fits) and where the expression
//! no longer overflows then. It changes the value, which is the point.
//!
//! Only integer literals count as constants (the originals also resolve named constants such as
//! `Integer.MAX_VALUE`). The expression is folded with Java's `int` and `long` semantics: `int` operations wrap at 32
//! bits unless an operand is a `long`. Like Error Prone, `MAX_VALUE + 1` and `MIN_VALUE - 1` are taken as intended,
//! and shifts are not taken as overflows. The outermost overflowing expression is reported.

use super::shared_arithmetic::{Value, fold, fold_with, integer_literal};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let text = context.text;
    let folded = fold(tree, text);
    if folded.value.is_none() || !folded.overflowed || is_in_overflowing_expression(context) {
        return;
    }
    let diagnostic = Diagnostic::new(Rule::ConstantOverflow, tree.start, "Constant expression overflows");
    diagnostics.push(match long_fix(tree, context) {
        Some(edit) => diagnostic.with_unsafe_fix(vec![edit]),
        None => diagnostic,
    });
}

/// Whether an enclosing operation overflows too, so that it is reported instead.
fn is_in_overflowing_expression(context: &Context) -> bool {
    let enclosing = context.ancestors.iter().rev().find(|t| !matches!(t.kind, K::Parens(_)));
    enclosing.is_some_and(|t| {
        matches!(t.kind, K::Binary { .. } | K::Unary { .. } | K::Cast { .. }) && {
            let folded = fold(t, context.text);
            folded.value.is_some() && folded.overflowed
        }
    })
}

/// `L` after the leftmost literal, if the expression initialises a `long` variable and no longer overflows then.
fn long_fix(tree: &Tree, context: &Context) -> Option<Edit> {
    let K::Vars(vars) = &context.parent()?.kind else { return None };
    let var = vars.iter().find(|v| v.init.as_deref().is_some_and(|i| std::ptr::eq(i, tree)))?;
    if !var.ty.as_deref().is_some_and(|t| matches!(&t.kind, K::Primitive(p) if p == "long")) {
        return None;
    }
    let mut leftmost = tree;
    while let K::Binary { lhs, .. } = &leftmost.kind {
        leftmost = lhs;
    }
    if !matches!(integer_literal(leftmost, context.text), Some(Value::Int(_))) {
        return None;
    }
    let folded = fold_with(tree, context.text, Some(leftmost));
    (folded.value.is_some() && !folded.overflowed).then(|| {
        // A replacement, as an insertion after a word is separated from it.
        Edit::new(leftmost.start..leftmost.end, format!("{}L", context.source(leftmost)))
    })
}
