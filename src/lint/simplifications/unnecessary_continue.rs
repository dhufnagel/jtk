//! S057: `continue` as the last thing a loop body does (IntelliJ UnnecessaryContinue, NetBeans
//! RemoveUnnecessaryContinue, Error Prone RedundantControlFlow): the loop continues anyway. The `continue` is the last
//! statement of the body, or of a branch of an `if` that is, and is unlabeled or names the label of that loop.
//!
//! The fix removes the statement (its whole line if it is alone on it), which changes nothing; it is not fixed where
//! it is the whole branch of an `if` (`if (a) continue;`). A loop whose whole body is `continue;` is not reported.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

use super::line_removal;

pub fn check(statement: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Continue(label) = &statement.kind else { return };
    let mut node = statement;
    for (index, parent) in context.ancestors.iter().enumerate().rev() {
        match &parent.kind {
            K::Block { stmts, .. } if stmts.last().is_some_and(|s| std::ptr::eq(s, node)) => {}
            K::If { then, else_, .. }
                if std::ptr::eq(&**then, node) || else_.as_deref().is_some_and(|e| std::ptr::eq(e, node)) => {}
            K::For { body, .. } | K::ForEach { body, .. } | K::While { body, .. } | K::DoWhile { body, .. }
                if std::ptr::eq(&**body, node) && !std::ptr::eq(node, statement) =>
            {
                let loop_label = match index.checked_sub(1).map(|i| &context.ancestors[i].kind) {
                    Some(K::Labeled { label, .. }) => Some(label),
                    _ => None,
                };
                if label.is_none() || label.as_ref() == loop_label {
                    report(statement, context, diagnostics);
                }
                return;
            }
            _ => return,
        }
        node = parent;
    }
}

fn report(statement: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let diagnostic = Diagnostic::new(Rule::UnnecessaryContinue, statement.start, "Unnecessary continue statement");
    let in_block = matches!(context.parent().map(|p| &p.kind), Some(K::Block { .. }));
    diagnostics.push(if in_block && !context.tokens().has_comment(statement.start..statement.end) {
        diagnostic.with_fix(vec![line_removal(statement.start..statement.end, context.text)])
    } else {
        diagnostic
    });
}
