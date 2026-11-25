//! K010: `Thread.interrupted()` in a block catching `InterruptedException` clears the interrupt flag instead of
//! restoring it with `Thread.currentThread().interrupt()` (Error Prone InterruptedInCatchBlock).
//!
//! Fix (safe, as in the catalogue): `Thread.currentThread().interrupt()`, only where the result of
//! `Thread.interrupted()` is unused. It does change the behaviour: the flag is set afterwards. But the flag is
//! normally already clear when the exception is caught (the JDK's blocking methods clear it before throwing), so the
//! original call does nothing, and restoring the flag is what it was clearly meant to do.
//!
//! `Thread` and `InterruptedException` must resolve to `java.lang` (subclasses of `InterruptedException` are not
//! recognised). Code in lambdas and classes declared in the catch block is not checked; a call in a nested catch of
//! `InterruptedException` is reported once, for the innermost catch.

use super::shared::{THREAD, for_each_in_same_code, method_call, names_type};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{Catch, K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Try { catches, .. } = &tree.kind else { return };
    for catch in catches.iter().filter(|c| catches_interrupted(c, context)) {
        for_each_in_same_code(&catch.block, &mut |child, parent| {
            if let K::Try { catches, .. } = &parent.kind
                && catches.iter().any(|c| std::ptr::eq(&*c.block, child) && catches_interrupted(c, context))
            {
                return false;
            }
            if let Some((Some(receiver), "interrupted", [])) = method_call(child)
                && names_type(receiver, THREAD, context)
            {
                diagnostics.push(diagnostic(child, receiver, parent, context));
            }
            true
        });
    }
}

fn catches_interrupted(catch: &Catch, context: &Context) -> bool {
    let catches = |ty: &Tree| names_type(ty, "java.lang.InterruptedException", context);
    catch.param.ty.as_deref().is_some_and(catches) || catch.union.iter().any(catches)
}

fn diagnostic(invocation: &Tree, receiver: &Tree, parent: &Tree, context: &Context) -> Diagnostic {
    let diagnostic = Diagnostic::new(
        Rule::InterruptedInCatch,
        invocation.start,
        "Thread.interrupted() clears the interrupt flag instead of restoring it",
    );
    let unused = matches!(parent.kind, K::ExprStmt(_));
    if !unused || context.tokens().has_comment(invocation.start..invocation.end) {
        return diagnostic;
    }
    let replacement = format!("{}.currentThread().interrupt()", context.source(receiver));
    diagnostic.with_fix(vec![Edit::new(invocation.start..invocation.end, replacement)])
}
