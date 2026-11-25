//! K058: a `ScheduledThreadPoolExecutor` with zero core threads: the pool keeps no thread to run scheduled tasks, so
//! they may run late or not at all (IntelliJ ScheduledThreadPoolExecutorWithZeroCoreThreads).
//!
//! No fix: the number of threads needed is not known.
//!
//! Reported are `new ScheduledThreadPoolExecutor(0, ...)` and `Executors.newScheduledThreadPool(0, ...)` with the
//! literal `0`, the classes resolved to `java.util.concurrent` through the imports. Unlike IntelliJ,
//! `setCorePoolSize(0)` is not checked.

use super::shared::{method_call, names_type};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (is_pool, args) = match &tree.kind {
        K::NewClass { ident, args, .. } => {
            (names_type(ident, "java.util.concurrent.ScheduledThreadPoolExecutor", context), &args[..])
        }
        _ => match method_call(tree) {
            Some((Some(receiver), "newScheduledThreadPool", args)) => {
                (names_type(receiver, "java.util.concurrent.Executors", context), args)
            }
            _ => return,
        },
    };
    let zero = args.first().is_some_and(|a| matches!(a.kind, K::Literal(LitKind::Int)) && context.source(a) == "0");
    if is_pool && zero {
        let message = "ScheduledThreadPoolExecutor with zero core threads";
        diagnostics.push(Diagnostic::new(Rule::ZeroCorePool, tree.start, message));
    }
}
