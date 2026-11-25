//! B205: a labeled statement among the statements of a `case` (`caes1: ...` or `defualt: ...`), most likely a
//! misspelt `case` or `default` (IntelliJ TextLabelInSwitchStatement, PMD NonCaseLabelInSwitch, NetBeans Possibly
//! missing switch "case" statement).
//!
//! Unlike PMD and IntelliJ, a label that a `break` or `continue` in its statement uses (a labeled loop to leave from
//! within the case) is not reported. No fix.

use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{CaseKind, K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Labeled { label, body } = &tree.kind else { return };
    let cases = match context.parent().map(|p| &p.kind) {
        Some(K::Switch { cases, .. } | K::SwitchExpr { cases, .. }) => cases,
        _ => return,
    };
    let in_case_statements =
        cases.iter().any(|c| c.kind == CaseKind::Statement && c.stmts.iter().any(|s| std::ptr::eq(s, tree)));
    if !in_case_statements || is_used(body, label) {
        return;
    }
    let message = format!("Label \"{label}\" in a switch, a misspelt case or default?");
    diagnostics.push(Diagnostic::new(Rule::LabelInSwitch, tree.start, message));
}

/// Whether a `break` or `continue` in the statement jumps to the label.
fn is_used(statement: &Tree, label: &str) -> bool {
    let mut used = false;
    for_each_descendant(statement, &mut |descendant, _| {
        used |= matches!(&descendant.kind, K::Break(Some(l)) | K::Continue(Some(l)) if l == label);
        !used
    });
    used
}
