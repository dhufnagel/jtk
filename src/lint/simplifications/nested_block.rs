//! S018: blocks nested directly in other blocks (Checkstyle AvoidNestedBlocks with the default
//! `allowInSwitchCase=false`, so also `case 1: { ... }`), which usually only limit the scope of variables.

use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(container: &Tree, _context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    match &container.kind {
        K::Block { stmts, .. } => report_blocks(stmts, diagnostics),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            for case in cases {
                report_blocks(&case.stmts, diagnostics);
            }
        }
        _ => {}
    }
}

fn report_blocks(statements: &[Tree], diagnostics: &mut Vec<Diagnostic>) {
    for block in statements.iter().filter(|s| matches!(s.kind, K::Block { .. })) {
        diagnostics.push(Diagnostic::new(Rule::NestedBlock, block.start, "Avoid nested blocks"));
    }
}
