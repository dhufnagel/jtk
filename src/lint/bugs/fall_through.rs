//! B004: a `case` with code that falls through to the next one (Checkstyle FallThrough, PMD
//! ImplicitSwitchFallThrough, Error Prone FallThrough). A comment like `// fall through` on the last non-empty line
//! before the next `case` marks it as intended.
//!
//! As in Checkstyle, a loop counts as ending the `case` if its body ends in `return` or `throw`, or if it is an
//! infinite loop without `break`.

use crate::ast::{Case, CaseKind, K, Tree};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};

/// Checkstyle's default relief pattern `falls?[ -]?thr(u|ough)`, spelled out.
const RELIEF_COMMENTS: &[&str] = &[
    "fallthru",
    "fall thru",
    "fall-thru",
    "fallthrough",
    "fall through",
    "fall-through",
    "fallsthru",
    "falls thru",
    "falls-thru",
    "fallsthrough",
    "falls through",
    "falls-through",
];

/// How a statement can leave the `case`.
#[derive(Clone, Copy)]
struct Exits {
    by_break: bool,
    by_continue: bool,
}

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (K::Switch { cases, .. } | K::SwitchExpr { cases, .. }) = &tree.kind else { return };
    for pair in cases.windows(2) {
        let [case, next] = pair else { continue };
        if case.kind != CaseKind::Statement {
            return;
        }
        let Some(last) = last_statement(&case.stmts) else { continue };
        if !ends_case(last, context.text) && !is_relieved(&context.text[last.end..next.start]) {
            diagnostics.push(Diagnostic::new(Rule::FallThrough, next.start, "Fall through from the previous case"));
        }
    }
}

/// Whether control cannot reach the end of a statement of a `case` (`case ...:`), so that the case does not fall
/// through: a `break` or `continue` leaves the case unless it targets a statement within it.
pub(crate) fn ends_case(statement: &Tree, text: &str) -> bool {
    terminates(statement, text, Exits { by_break: true, by_continue: true }, &mut Vec::new(), 0)
}

/// Whether the last non-empty line between the case's code and the next `case` has a relief comment.
fn is_relieved(between: &str) -> bool {
    let last_line = between.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
    RELIEF_COMMENTS.iter().any(|c| last_line.contains(c))
}

/// The last statement that is not empty (`;`).
fn last_statement(statements: &[Tree]) -> Option<&Tree> {
    statements.iter().rfind(|s| !matches!(s.kind, K::Empty))
}

/// Whether control cannot reach the end of the statement. `labels` are those of the labeled statements between the
/// case and the statement, the last `direct` of which label the statement itself.
fn terminates<'t>(statement: &'t Tree, text: &str, exits: Exits, labels: &mut Vec<&'t str>, direct: usize) -> bool {
    match &statement.kind {
        K::Return(_) | K::Throw(_) | K::Yield(_) => true,
        K::Break(label) => jumps_out(label.as_deref(), exits.by_break, labels),
        K::Continue(label) => jumps_out(label.as_deref(), exits.by_continue, labels),
        K::Block { stmts, .. } => last_statement(stmts).is_some_and(|s| terminates(s, text, exits, labels, 0)),
        K::If { then, else_: Some(else_), .. } => {
            terminates(then, text, exits, labels, 0) && terminates(else_, text, exits, labels, 0)
        }
        K::Try { block, catches, finally, .. } => {
            finally.as_deref().is_some_and(|f| terminates(f, text, exits, labels, 0))
                || terminates(block, text, exits, labels, 0)
                    && catches.iter().all(|c| terminates(&c.block, text, exits, labels, 0))
        }
        K::Synchronized { block, .. } => terminates(block, text, exits, labels, 0),
        K::Labeled { label, body } => {
            labels.push(label);
            let terminates = terminates(body, text, exits, labels, direct + 1);
            labels.pop();
            terminates
        }
        K::While { body, .. } | K::DoWhile { body, .. } | K::For { body, .. } | K::ForEach { body, .. } => {
            let is_infinite = match &statement.kind {
                K::For { cond, .. } => cond.as_deref().is_none_or(|c| is_true(c, text)),
                K::While { cond, .. } | K::DoWhile { cond, .. } => is_true(cond, text),
                _ => false,
            };
            let has_break = has_break(body, &labels[labels.len() - direct..]);
            (is_infinite && !has_break)
                || terminates(body, text, Exits { by_break: false, by_continue: false }, labels, 0)
        }
        K::Switch { cases, .. } => switch_terminates(cases, text, exits, labels),
        K::ExprStmt(expression) => match &expression.kind {
            K::SwitchExpr { cases, .. } => switch_terminates(cases, text, exits, labels),
            _ => false,
        },
        _ => false,
    }
}

/// Whether a `break` or `continue` leaves the case: an unlabeled one if `unlabeled_leaves`, a labeled one unless it
/// targets a statement within the case.
fn jumps_out(label: Option<&str>, unlabeled_leaves: bool, labels: &[&str]) -> bool {
    label.map_or(unlabeled_leaves, |l| !labels.contains(&l))
}

/// Whether a `break` leaves the loop with the body: an unlabeled one outside nested loops and switches, or one naming
/// one of the loop's labels.
fn has_break(body: &Tree, labels: &[&str]) -> bool {
    match &body.kind {
        K::Break(label) => label.as_deref().is_none_or(|l| labels.contains(&l)),
        _ => any_child(body, &mut |child| breaks(child, labels, is_switch(body))),
    }
}

/// Whether the tree is or contains a `break` that leaves the loop: `in_target` if a nested loop or switch catches
/// unlabeled ones.
fn breaks(tree: &Tree, labels: &[&str], in_target: bool) -> bool {
    match &tree.kind {
        K::Break(None) => !in_target,
        K::Break(Some(label)) => labels.contains(&label.as_str()),
        K::Lambda { .. } | K::Class(_) => false,
        K::NewClass { outer, args, .. } => outer.iter().map(|o| &**o).chain(args).any(|t| breaks(t, labels, in_target)),
        _ => {
            let is_loop = matches!(tree.kind, K::For { .. } | K::ForEach { .. } | K::While { .. } | K::DoWhile { .. });
            let in_target = in_target || is_loop || is_switch(tree);
            any_child(tree, &mut |child| breaks(child, labels, in_target))
        }
    }
}

fn is_switch(tree: &Tree) -> bool {
    matches!(tree.kind, K::Switch { .. } | K::SwitchExpr { .. })
}

fn any_child<'t>(tree: &'t Tree, predicate: &mut impl FnMut(&'t Tree) -> bool) -> bool {
    let mut found = false;
    tree.for_each_child(&mut |child| found = found || predicate(child));
    found
}

/// A nested `switch` ends the case if it has a `default` and all its cases end it (a `break` only leaves the nested
/// switch).
fn switch_terminates<'t>(cases: &'t [Case], text: &str, exits: Exits, labels: &mut Vec<&'t str>) -> bool {
    let nested = Exits { by_break: false, ..exits };
    cases.iter().any(|c| c.is_default)
        && cases.iter().all(|c| {
            c.kind == CaseKind::Statement
                && last_statement(&c.stmts).is_some_and(|s| terminates(s, text, nested, labels, 0))
        })
}

fn is_true(condition: &Tree, text: &str) -> bool {
    let condition = strip_parens(condition);
    &text[condition.start..condition.end] == "true"
}
