//! K042: a static field written while holding only an instance lock: other instances hold other locks, so the writes
//! are not synchronised with each other (IntelliJ AccessToStaticFieldLockedOnInstance, Error Prone
//! StaticGuardedByInstance).
//!
//! No fix.
//!
//! Instance locks are synchronized instance methods, `synchronized (this)` and `synchronized` on an instance field
//! declared in the file. Writes are assignments, compound assignments and increments of a static field declared in
//! the file, by name (`count`) or qualified with an enclosing class (`Counter.count`). Unlike IntelliJ, reads are not
//! reported. Code in nested `synchronized` blocks (checked on their own), lambdas and classes is skipped, and blocks
//! inside a `synchronized (X.class)` block are not reported.

use super::shared::{
    Variable, declared_field, enclosing_in_same_code, for_each_in_same_code, variable_below, variable_of,
};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Modifier, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let region = match &tree.kind {
        K::Method(method) if method.mods.has(Modifier::Synchronized) && !method.mods.has(Modifier::Static) => {
            method.body.as_deref()
        }
        K::Synchronized { lock, block } if is_instance_lock(lock, context) && !in_class_lock(tree, context) => {
            Some(&**block)
        }
        _ => None,
    };
    let Some(region) = region else { return };
    for_each_in_same_code(region, &mut |child, _| {
        let target = match &child.kind {
            K::Assign { lhs, .. } => Some(strip_parens(lhs)),
            K::Unary { op, expr, .. } if op == "++" || op == "--" => Some(strip_parens(expr)),
            _ => None,
        };
        if let Some(field) = target.and_then(|t| static_field(tree, t, context)) {
            let message = format!("Static field '{}' written under an instance lock", field.decl.name);
            diagnostics.push(Diagnostic::new(Rule::StaticFieldInstanceLock, child.start, message));
        }
        !matches!(child.kind, K::Synchronized { .. })
    });
}

/// Whether the lock is `this` or an instance field declared in the file.
fn is_instance_lock(lock: &Tree, context: &Context) -> bool {
    let lock = strip_parens(lock);
    matches!(&lock.kind, K::Ident(name) if name == "this")
        || variable_of(lock, &context.ancestors, context).is_some_and(|v| v.is_field() && !v.is_static())
}

/// Whether the tree is inside a `synchronized (X.class)` block in the same code.
fn in_class_lock(tree: &Tree, context: &Context) -> bool {
    enclosing_in_same_code(tree, context).any(|(parent, child)| match &parent.kind {
        K::Synchronized { lock, block } => {
            std::ptr::eq(&**block, child)
                && matches!(&strip_parens(lock).kind, K::MemberSelect { name, .. } if name == "class")
        }
        _ => false,
    })
}

/// The static field written by the target of an assignment, if it is declared in the file.
fn static_field<'t>(root: &'t Tree, target: &'t Tree, context: &Context<'t>) -> Option<Variable<'t>> {
    let field = match &target.kind {
        K::Ident(_) => variable_below(root, target, context),
        K::MemberSelect { expr, name } => {
            let K::Ident(class_name) = &expr.kind else { return None };
            enclosing_class_field(class_name, name, context)
        }
        _ => None,
    }?;
    field.is_static().then_some(field)
}

/// The field declared by the enclosing class with the simple name.
fn enclosing_class_field<'t>(class_name: &str, field: &str, context: &Context<'t>) -> Option<Variable<'t>> {
    let index = context.ancestors.iter().rposition(|t| matches!(&t.kind, K::Class(c) if c.name == class_name))?;
    let K::Class(class) = &context.ancestors[index].kind else { return None };
    declared_field(class, field)
}
