//! K006: `Thread.stop()`, `suspend()` and `resume()` are deprecated and unsafe: stopping releases all monitors in an
//! inconsistent state, suspending keeps them and invites deadlocks; since Java 20 they throw
//! `UnsupportedOperationException` (IntelliJ ThreadStopSuspendResume, NetBeans "Invoking Thread.stop()/suspend()/
//! resume()").
//!
//! No fix. Without type information, only receivers known to be a `java.lang.Thread` are checked (see K001).

use super::shared::{is_thread, method_call};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::Tree;

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some((Some(receiver), name @ ("stop" | "suspend" | "resume"), [])) = method_call(tree) else { return };
    if is_thread(receiver, &context.ancestors, context) {
        diagnostics.push(Diagnostic::new(Rule::ThreadStop, tree.start, format!("Thread.{name}() is unsafe")));
    }
}
