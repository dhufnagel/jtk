//! K016: `wait()` or `Condition.await()` outside a loop: because of spurious wakeups (and notifications for other
//! conditions), the condition waited for must be checked again after waking up (Error Prone WaitNotInLoop, IntelliJ
//! WaitNotInLoop, AwaitNotInLoop and UnconditionalWait).
//!
//! No fix: turning an enclosing `if` into `while` is only right for the simple "wait until a condition holds" case.
//!
//! `wait` is final in `Object`, so any `wait()` is `Object.wait()`; with arguments (`wait(timeout)`), only where the
//! file declares no method named `wait` that could be an overload. `await`, `awaitNanos`, `awaitUntil` and
//! `awaitUninterruptibly` only on receivers known to be a `java.util.concurrent.locks.Condition` (a local variable,
//! field or parameter declared in the file): `CountDownLatch.await()` needs no loop. A loop counts only in the same
//! code: not around a lambda or class containing the call.

use super::shared::{CONDITION, enclosing_in_same_code, has_type, method_call};
use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some((receiver, name, args)) = method_call(tree) else { return };
    let is_wait = match name {
        "wait" => args.is_empty() || args.len() <= 2 && !declares_wait_method(context),
        "await" | "awaitNanos" | "awaitUntil" | "awaitUninterruptibly" => {
            receiver.is_some_and(|r| has_type(r, &context.ancestors, &[CONDITION], context))
        }
        _ => false,
    };
    let in_loop = || {
        enclosing_in_same_code(tree, context).any(|(parent, _)| {
            matches!(parent.kind, K::While { .. } | K::DoWhile { .. } | K::For { .. } | K::ForEach { .. })
        })
    };
    if is_wait && !in_loop() {
        diagnostics.push(Diagnostic::new(Rule::WaitNotInLoop, tree.start, format!("{name}() not called in a loop")));
    }
}

/// Whether the file declares a method named `wait`.
fn declares_wait_method(context: &Context) -> bool {
    let Some(unit) = context.ancestors.first() else { return true };
    let mut found = false;
    for_each_descendant(unit, &mut |tree, _| {
        found |= matches!(&tree.kind, K::Method(method) if method.name == "wait");
        !found
    });
    found
}
