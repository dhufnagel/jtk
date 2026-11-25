//! K024: `lock.lock()` not directly followed by a `try` whose `finally` releases the lock: if a statement in between
//! throws, or the unlock is not in a `finally`, the lock is never released; a `lock()` inside the `try` releases a
//! lock that was not acquired if `lock()` throws (Error Prone LockNotBeforeTry, IntelliJ SafeLock, NetBeans "Lock not
//! unlocked in finally").
//!
//! No fix.
//!
//! As in Error Prone, only a `lock()` statement directly in a block is checked, and only where the lock is visibly
//! released in the same block: by the `finally` of the nearest enclosing `try` around it, or by a later `try` with a
//! `finally` or a later `unlock()` statement in the block (a second `lock()` of the same lock counts too). Locks
//! released elsewhere (another method) are not reported. A `finally` that also unlocks another lock does not count as
//! releasing it. Locks are compared by their source. Without type information, only receivers known to be a `Lock`,
//! `ReentrantLock` or one of the locks of a `ReentrantReadWriteLock` are checked: variables declared with these types
//! in the file, and `readLock()`/`writeLock()` of variables declared as `ReadWriteLock` or `ReentrantReadWriteLock`.

use super::shared::{enclosing_in_same_code, for_each_in_same_code, has_type, method_call, normalized};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

const LOCKS: &[&str] = &[
    "java.util.concurrent.locks.Lock",
    "java.util.concurrent.locks.ReentrantLock",
    "java.util.concurrent.locks.ReentrantReadWriteLock.ReadLock",
    "java.util.concurrent.locks.ReentrantReadWriteLock.WriteLock",
];

const READ_WRITE_LOCKS: &[&str] =
    &["java.util.concurrent.locks.ReadWriteLock", "java.util.concurrent.locks.ReentrantReadWriteLock"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some((Some(receiver), "lock", [])) = method_call(tree) else { return };
    let (Some(statement), Some(block)) = (context.parent(), context.grandparent()) else { return };
    let (K::ExprStmt(_), K::Block { stmts, .. }) = (&statement.kind, &block.kind) else { return };
    if !is_lock(receiver, context) {
        return;
    }
    let lock = normalized(receiver, context);
    let index = stmts.iter().position(|s| std::ptr::eq(s, statement)).unwrap_or(stmts.len());
    let following = stmts.get(index + 1..).unwrap_or_default();
    if following.first().is_some_and(|s| matches!(s.kind, K::Try { .. })) {
        return;
    }
    let enclosing_try = enclosing_in_same_code(tree, context).find(|(parent, _)| matches!(parent.kind, K::Try { .. }));
    let message = match enclosing_try {
        Some((parent @ Tree { kind: K::Try { block, .. }, .. }, child))
            if std::ptr::eq(&**block, child) && releases(parent, &lock, context) =>
        {
            "lock() inside the try block that releases the lock"
        }
        _ if following.iter().any(|s| is_released_by(s, &lock, context)) => {
            "lock() not directly followed by a try block that releases the lock in finally"
        }
        _ => return,
    };
    diagnostics.push(Diagnostic::new(Rule::LockWithoutTry, tree.start, message));
}

/// Whether the receiver is known to be a lock.
fn is_lock(receiver: &Tree, context: &Context) -> bool {
    match method_call(strip_parens(receiver)) {
        Some((Some(read_write), "readLock" | "writeLock", [])) => {
            has_type(read_write, &context.ancestors, READ_WRITE_LOCKS, context)
        }
        _ => has_type(receiver, &context.ancestors, LOCKS, context),
    }
}

/// Whether a statement following the `lock()` shows that the lock is released (or acquired again) in the block.
fn is_released_by(statement: &Tree, lock: &str, context: &Context) -> bool {
    match &statement.kind {
        K::Try { .. } => releases(statement, lock, context),
        K::ExprStmt(expression) => {
            matches!(method_call(expression), Some((Some(r), "lock" | "unlock", [])) if normalized(r, context) == lock)
        }
        _ => false,
    }
}

/// Whether the `finally` of the `try` unlocks the lock and no other one.
fn releases(try_: &Tree, lock: &str, context: &Context) -> bool {
    let K::Try { finally: Some(finally), .. } = &try_.kind else { return false };
    let (mut this, mut other) = (false, false);
    for_each_in_same_code(finally, &mut |tree, _| {
        if let Some((Some(receiver), "unlock", [])) = method_call(tree) {
            if normalized(receiver, context) == lock { this = true } else { other = true }
        }
        true
    });
    this && !other
}
