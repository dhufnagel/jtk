//! B032: a `switch` over a reference type without `case null` (Checkstyle MissingNullCaseInSwitch): it throws a
//! `NullPointerException` for `null`.
//!
//! As in Checkstyle, a switch counts as over a reference type if a `case` has a pattern or a string literal (not a
//! text block), and only the first constant of a `case` is checked for `null` (`case null, default` but not
//! `case "a", null`).

use crate::ast::{K, LitKind, Tree};
use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (K::Switch { cases, .. } | K::SwitchExpr { cases, .. }) = &tree.kind else { return };
    // The cases with `case` (not `default` alone).
    let cases = || cases.iter().filter(|c| !c.is_default);
    let has_null = cases().any(|c| c.labels.first().is_some_and(|l| matches!(l.kind, K::Literal(LitKind::Null))));
    let has_reference = || {
        cases().any(|c| {
            c.labels.iter().any(|l| matches!(l.kind, K::BindingPattern { .. } | K::RecordPattern { .. }))
                || c.labels.iter().chain(c.guard.as_deref()).any(|l| has_string(l, context.text))
        })
    };
    if !has_null && has_reference() {
        diagnostics.push(Diagnostic::new(
            Rule::MissingNullCase,
            tree.start,
            "Switch using reference types should have a null case",
        ));
    }
}

/// Whether the tree is or contains a string literal (not a text block).
fn has_string(tree: &Tree, text: &str) -> bool {
    let is_string = |t: &Tree| matches!(t.kind, K::Literal(LitKind::String)) && !text[t.start..].starts_with("\"\"\"");
    let mut found = is_string(tree);
    for_each_descendant(tree, &mut |t, _| {
        found = found || is_string(t);
        !found
    });
    found
}
