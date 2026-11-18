//! B006: a `default` label before other `case`s of the switch (Checkstyle DefaultComesLast with its default
//! properties, PMD DefaultLabelNotLastInSwitch): readers expect the fallback at the end.
//!
//! As in Checkstyle, `default` may share its statements with `case` labels in any order if they are the last ones
//! (`default: case 1: ...`), and `case null, default` is not checked.

use crate::ast::{Case, CaseKind, K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, _context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (K::Switch { cases, .. } | K::SwitchExpr { cases, .. }) = &tree.kind else { return };
    // Checkstyle's case groups: each label is a case of its own, labels that share statements have none.
    let ends_case_group =
        |case: &Case| case.kind == CaseKind::Rule || case.stmts.iter().any(|s| !matches!(s.kind, K::Empty));
    for (index, case) in cases.iter().enumerate().filter(|(_, c)| c.is_default) {
        let group_end = cases[index..].iter().position(ends_case_group).map_or(cases.len() - 1, |p| index + p);
        if group_end + 1 < cases.len() {
            diagnostics.push(Diagnostic::new(
                Rule::DefaultComesLast,
                case.start,
                "Default should be last label in the switch",
            ));
        }
    }
}
