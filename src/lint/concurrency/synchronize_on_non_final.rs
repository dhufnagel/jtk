//! K026: `synchronized` on a field that is not final: if the field is reassigned, threads lock different objects and
//! no longer exclude each other (IntelliJ SynchronizeOnNonFinalField, Error Prone SynchronizeOnNonFinalField,
//! NetBeans "Synchronization on non-final field").
//!
//! No fix: making the field final needs it to be assigned exactly once.
//!
//! Only fields declared in the file are checked, by name (`lock`) or on `this` (`this.lock`); static fields too (as in
//! IntelliJ, unlike Error Prone). Fields annotated `@LazyInit` are skipped, as in Error Prone.

use super::shared::variable_of;
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Synchronized { lock, .. } = &tree.kind else { return };
    let lock = strip_parens(lock);
    let Some(field) = variable_of(lock, &context.ancestors, context).filter(|v| v.is_field() && !v.is_final()) else {
        return;
    };
    let lazy = field.decl.mods.annotations.iter().any(|a| match &a.kind {
        K::Annotation { ty, .. } => context.source(ty).rsplit('.').next() == Some("LazyInit"),
        _ => false,
    });
    if !lazy {
        let message = format!("Synchronization on non-final field '{}'", field.decl.name);
        diagnostics.push(Diagnostic::new(Rule::SynchronizeOnNonFinal, lock.start, message));
    }
}
