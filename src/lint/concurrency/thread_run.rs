//! K001: `thread.run()` runs the thread's task in the calling thread instead of starting the thread (IntelliJ
//! ThreadRun, NetBeans "Invoking Thread.run()", PMD DontCallThreadRun).
//!
//! Unsafe fix: `thread.start()`, which runs the task concurrently, so it changes the behaviour (intentionally).
//!
//! Without type information, only receivers known to be a `java.lang.Thread` are checked: `new Thread(...)`, a cast to
//! `Thread`, `Thread.currentThread()` and variables declared as `Thread` in the file. Subclasses of `Thread` are not
//! recognised, and `super.run()` is no violation.

use super::shared::{is_thread, method_call};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some((Some(receiver), "run", [])) = method_call(tree) else { return };
    if !is_thread(receiver, &context.ancestors, context) {
        return;
    }
    let K::MethodInvocation { select, .. } = &tree.kind else { return };
    let name = select.end - "run".len()..select.end;
    let diagnostic = Diagnostic::new(Rule::ThreadRun, tree.start, "Thread.run() instead of start()");
    diagnostics.push(if context.text.get(name.clone()) == Some("run") {
        diagnostic.with_unsafe_fix(vec![Edit::new(name, "start")])
    } else {
        diagnostic
    });
}
