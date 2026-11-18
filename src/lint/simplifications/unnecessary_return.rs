//! S010: a `return;` that is the last thing a method, constructor, initializer or lambda does (PMD
//! UnnecessaryReturn): every statement enclosing it is the last of its parent (or the branch of an `if` or the body of
//! a `try`), outside loops and switch expressions.
//!
//! The fix removes the statement (its whole line if it is alone on it) when it is in a block or a `case` group;
//! elsewhere (`if (a) return;`) removing it would leave a statement without body.

use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

use super::line_removal;

pub fn check(statement: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if !matches!(statement.kind, K::Return(None)) {
        return;
    }
    let mut node = statement;
    for parent in context.ancestors.iter().rev() {
        if matches!(parent.kind, K::Method(_) | K::Lambda { .. } | K::Class(_) | K::NewClass { .. }) {
            let diagnostic = Diagnostic::new(Rule::UnnecessaryReturn, statement.start, "Unnecessary return statement");
            let container = context.parent().map(|p| &p.kind);
            let removable = matches!(container, Some(K::Block { .. } | K::Switch { .. } | K::SwitchExpr { .. }));
            let has_comment = context.tokens().has_comment(statement.start..statement.end);
            diagnostics.push(if removable && !has_comment {
                diagnostic.with_fix(vec![line_removal(statement.start..statement.end, context.text)])
            } else {
                diagnostic
            });
            return;
        }
        if is_statement(node) && !is_last_statement(node, parent) {
            return;
        }
        node = parent;
    }
}

fn is_statement(tree: &Tree) -> bool {
    !tree.is_expression() || matches!(tree.kind, K::SwitchExpr { .. })
}

/// Whether the statement is the last thing its parent does before control leaves it.
fn is_last_statement(statement: &Tree, parent: &Tree) -> bool {
    match &parent.kind {
        K::Block { stmts, .. } => is_last(statement, stmts),
        K::While { .. } | K::DoWhile { .. } | K::For { .. } | K::ForEach { .. } => false,
        // A rule's body ends the switch statement; the last statement of the last group does.
        K::Switch { cases, .. } => {
            match cases.iter().position(|c| c.stmts.iter().any(|s| std::ptr::eq(s, statement))) {
                Some(index) => index == cases.len() - 1 && is_last(statement, &cases[index].stmts),
                None => true,
            }
        }
        K::SwitchExpr { .. } => false,
        _ => true,
    }
}

/// Whether only empty statements follow the statement.
fn is_last(statement: &Tree, statements: &[Tree]) -> bool {
    let index = statements.iter().position(|s| std::ptr::eq(s, statement));
    index.is_some_and(|i| statements[i + 1..].iter().all(|s| matches!(s.kind, K::Empty)))
}
