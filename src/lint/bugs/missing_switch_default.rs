//! B005: a `switch` statement without `default` (Checkstyle MissingSwitchDefault, PMD SwitchStmtsShouldHaveDefault
//! and NonExhaustiveSwitch): a value no `case` handles is silently ignored.
//!
//! As in Checkstyle, switch expressions are not checked (the compiler checks that they are exhaustive), nor are
//! switch statements with an unguarded pattern or a `case null`, which must be exhaustive too.

use crate::ast::{K, LitKind, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, _context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Switch { cases, .. } = &tree.kind else { return };
    let is_exempt = cases.iter().any(|case| {
        let has_pattern = case.guard.is_none()
            && case.labels.iter().any(|l| matches!(l.kind, K::BindingPattern { .. } | K::RecordPattern { .. }));
        let has_null = case.labels.iter().any(|l| matches!(l.kind, K::Literal(LitKind::Null)));
        let has_default = case.is_default || case.labels.iter().any(|l| matches!(l.kind, K::DefaultCaseLabel));
        has_default || has_pattern || has_null
    });
    if !is_exempt {
        diagnostics.push(Diagnostic::new(Rule::MissingSwitchDefault, tree.start, "switch without \"default\" clause"));
    }
}
