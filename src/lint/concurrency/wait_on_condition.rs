//! K023: `wait()`, `notify()` or `notifyAll()` on a `Condition` uses the condition's monitor, not the condition: meant
//! are `await()`, `signal()` and `signalAll()` (NetBeans "Invoking Condition.notify()" and "Invoking
//! Condition.wait()", IntelliJ NotifyCalledOnCondition and WaitCalledOnCondition).
//!
//! Unsafe fix: the `Condition` method, for calls without arguments (`wait(timeout)` has no direct counterpart). It
//! changes the behaviour: the lock of the condition must then be held instead of the monitor.
//!
//! Only receivers known to be a `java.util.concurrent.locks.Condition` (a local variable, field or parameter declared
//! in the file) are checked.

use super::shared::{CONDITION, has_type, method_call};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some((Some(receiver), name, args)) = method_call(tree) else { return };
    let replacement = match (name, args.len()) {
        ("wait", 0..=2) => "await",
        ("notify", 0) => "signal",
        ("notifyAll", 0) => "signalAll",
        _ => return,
    };
    if !has_type(receiver, &context.ancestors, &[CONDITION], context) {
        return;
    }
    let message = format!("{name}() on a Condition instead of {replacement}()");
    let diagnostic = Diagnostic::new(Rule::WaitOnCondition, tree.start, message);
    let K::MethodInvocation { select, .. } = &tree.kind else { return };
    let name_range = select.end - name.len()..select.end;
    diagnostics.push(if args.is_empty() && context.text.get(name_range.clone()) == Some(name) {
        diagnostic.with_unsafe_fix(vec![Edit::new(name_range, replacement)])
    } else {
        diagnostic
    });
}
