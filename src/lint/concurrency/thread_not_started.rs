//! K002: a thread created in an expression statement (`new Thread(task);`) is never started (Error Prone
//! DeadThread).
//!
//! No fix: appending `.start()` (as Error Prone suggests) would run code that never ran before.
//!
//! The created class must be `java.lang.Thread` (resolved through the imports); subclasses are not recognised.

use super::shared::{THREAD, names_type};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::ExprStmt(expression) = &tree.kind else { return };
    let creation = strip_parens(expression);
    let K::NewClass { ident, .. } = &creation.kind else { return };
    if names_type(ident, THREAD, context) {
        diagnostics.push(Diagnostic::new(Rule::ThreadNotStarted, creation.start, "Thread created but never started"));
    }
}
