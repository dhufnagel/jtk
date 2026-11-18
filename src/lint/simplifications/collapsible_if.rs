//! S004: an `if` whose only statement is another `if`, neither with an `else` (PMD CollapsibleIfStatements):
//! `if (a) { if (b) s }` can be `if (a && b) s`. Reported on the inner `if`, like PMD.
//!
//! The fix joins the conditions, parenthesizing those whose operators bind less tightly than `&&` (`||`, `?:`,
//! assignments), and keeps the inner statement. It is left out if a comment outside the inner statement would be lost.

use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(outer: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::If { cond: outer_condition, then, else_: None } = &outer.kind else { return };
    let inner = match &then.kind {
        K::If { .. } => then,
        K::Block { stmts, .. } => {
            let mut statements = stmts.iter().filter(|s| !matches!(s.kind, K::Empty));
            match (statements.next(), statements.next()) {
                (Some(single), None) if matches!(single.kind, K::If { .. }) => single,
                _ => return,
            }
        }
        _ => return,
    };
    let K::If { cond: inner_condition, then: statement, else_: None } = &inner.kind else { return };
    let diagnostic =
        Diagnostic::new(Rule::CollapsibleIf, inner.start, "This if statement could be combined with its parent");
    let tokens = context.tokens();
    let keeps_comments =
        !tokens.has_comment(outer.start..statement.start) && !tokens.has_comment(statement.end..outer.end);
    diagnostics.push(if keeps_comments {
        let replacement = format!(
            "if ({} && {}) {}",
            condition(outer_condition, context),
            condition(inner_condition, context),
            context.source(statement)
        );
        diagnostic.with_fix(vec![Edit::new(outer.start..outer.end, replacement)])
    } else {
        diagnostic
    });
}

/// The condition of an `if` statement (in its parentheses) as an operand of `&&`.
fn condition(parenthesized: &Tree, context: &Context) -> String {
    let K::Parens(expression) = &parenthesized.kind else { return context.source(parenthesized).to_string() };
    let code = context.source(expression);
    let binds_less = match &expression.kind {
        K::Binary { op, .. } => op == "||",
        K::Conditional { .. } | K::Assign { .. } | K::Lambda { .. } | K::SwitchExpr { .. } => true,
        _ => false,
    };
    if binds_less { format!("({code})") } else { code.to_string() }
}
