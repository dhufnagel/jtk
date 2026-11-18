//! B012: an assignment inside an expression, such as `a = b = c` or `f(x = 1)` (Checkstyle InnerAssignment, PMD
//! AssignmentInOperand): the side effect is easy to overlook.
//!
//! As in Checkstyle, assignments are allowed as statements (also as the body of a control statement without braces
//! or of a rule of a `switch` statement), as lambda bodies, in the init and update parts of `for`, and as the operand
//! of a comparison in a loop condition (`while ((line = r.readLine()) != null)`), also combined with `&&`, `||`,
//! `&`, `|` and `!`. An assignment statement after a label is reported, as in Checkstyle.

use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Assign { lhs, rhs, .. } = &tree.kind else { return };
    // An annotation's `k = v` is no assignment.
    if context.parent().is_some_and(|p| matches!(p.kind, K::Annotation { .. })) || is_allowed(tree, &context.ancestors)
    {
        return;
    }
    let offset = context.tokens().in_range(lhs.end..rhs.start).next().map_or(tree.start, |o| o.start);
    diagnostics.push(Diagnostic::new(Rule::InnerAssignment, offset, "Inner assignments should be avoided"));
}

/// Whether the assignment is in one of the contexts Checkstyle allows.
fn is_allowed(assignment: &Tree, ancestors: &[&Tree]) -> bool {
    // Checkstyle's syntax tree has no nodes for parentheses: the assignment with its parentheses is the child.
    let mut child = assignment;
    let mut index = ancestors.len();
    while index > 0 && matches!(ancestors[index - 1].kind, K::Parens(_)) {
        index -= 1;
        child = ancestors[index];
    }
    let Some(parent_index) = index.checked_sub(1) else { return false };
    let grandparent = parent_index.checked_sub(1).map(|i| ancestors[i]);
    match &ancestors[parent_index].kind {
        K::ExprStmt(_) => grandparent.is_some_and(|g| {
            is_statement_list(g)
                || matches!(
                    g.kind,
                    K::If { .. } | K::While { .. } | K::DoWhile { .. } | K::For { .. } | K::ForEach { .. }
                )
        }),
        // The expression of a rule of a switch statement.
        K::Switch { cases, .. } => {
            cases.iter().any(|c| c.body.as_deref().is_some_and(|b| std::ptr::eq(b, child)))
                && grandparent.is_some_and(is_statement_list)
        }
        K::Lambda { .. } => true,
        K::For { update, .. } => update.iter().any(|u| std::ptr::eq(u, child)),
        K::Binary { op, .. } if matches!(op.as_str(), "==" | "!=" | "<" | "<=" | ">" | ">=") => {
            is_loop_condition(ancestors, parent_index)
        }
        _ => false,
    }
}

/// Checkstyle's statement lists: blocks and the statements of a `case`.
fn is_statement_list(tree: &Tree) -> bool {
    matches!(tree.kind, K::Block { .. } | K::Switch { .. } | K::SwitchExpr { .. })
}

/// Whether the comparison (at `index` of the ancestors) is the condition of a loop, possibly combined by `&&`, `||`,
/// `&`, `|` and `!`.
fn is_loop_condition(ancestors: &[&Tree], mut index: usize) -> bool {
    while index > 0 {
        let ancestor = ancestors[index - 1];
        let is_logical = match &ancestor.kind {
            K::Parens(_) => true,
            K::Binary { op, .. } => matches!(op.as_str(), "&&" | "||" | "&" | "|"),
            K::Unary { op, .. } => op == "!",
            _ => false,
        };
        if !is_logical {
            return match &ancestor.kind {
                K::While { .. } | K::DoWhile { .. } => true,
                K::For { cond, .. } => cond.as_deref().is_some_and(|c| std::ptr::eq(c, ancestors[index])),
                _ => false,
            };
        }
        index -= 1;
    }
    false
}
