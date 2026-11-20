//! B024: a `clone()` or `finalize()` that does not call `super.clone()` or `super.finalize()` (Checkstyle SuperClone
//! and SuperFinalize, PMD ProperCloneImplementation and FinalizeDoesNotCallSuperFinalize).
//!
//! As in Checkstyle: only the overloads without parameters count, not native ones nor those of interfaces; an
//! abstract method is reported too (it has no call); the call must be unqualified (`super.clone()` without
//! arguments, or `super::clone`) and belong to the method itself, which a lambda does but a constructor, initializer
//! or other method (also of a nested class) does not.

use super::avoided_methods::{declared_method, is_parameterless};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Modifier, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(method) = declared_method(tree, context) else { return };
    let Some(name) = ["clone", "finalize"].into_iter().find(|n| is_parameterless(method, n)) else { return };
    let in_interface =
        context.parent().is_some_and(|p| matches!(&p.kind, K::Class(c) if c.kind == ClassKind::Interface));
    if in_interface
        || method.mods.has(Modifier::Native)
        || method.body.as_deref().is_some_and(|body| calls_super(body, name))
    {
        return;
    }
    diagnostics.push(Diagnostic::new(
        Rule::MissingSuperCall,
        method.name_pos,
        format!("Method '{name}' should call 'super.{name}'"),
    ));
}

/// Whether the code calls the super method of the name, outside nested methods, constructors and instance
/// initializers.
fn calls_super(tree: &Tree, name: &str) -> bool {
    let mut found = false;
    tree.for_each_child(&mut |child| {
        found = found
            // Like tree-sitter's kinds of declarations: a compact constructor is not one of them.
            || !matches!(&child.kind, K::Method(m) if !m.compact_ctor)
                && !is_instance_initializer(tree, child)
                && (is_super_call(child, name) || calls_super(child, name));
    });
    found
}

/// Whether `child` is an instance initializer in a class body of `parent` (not in an interface or enum body).
fn is_instance_initializer(parent: &Tree, child: &Tree) -> bool {
    let in_class_body = match &parent.kind {
        K::Class(class) => match class.kind {
            ClassKind::Class | ClassKind::Record => true,
            // The body of an enum constant, not the enum's own members.
            ClassKind::Enum => class.members.first().is_none_or(|m| child.start < m.start),
            ClassKind::Interface | ClassKind::Annotation => false,
        },
        K::NewClass { .. } => true,
        _ => false,
    };
    in_class_body && matches!(child.kind, K::Block { is_static: false, .. })
}

/// `super.name()` without arguments or `super::name`.
fn is_super_call(tree: &Tree, name: &str) -> bool {
    match &tree.kind {
        K::MethodInvocation { select, args, .. } => {
            args.is_empty() && matches!(&select.kind, K::MemberSelect { expr, name: n } if n == name && is_super(expr))
        }
        K::MemberRef { qual, name: Some(n), .. } => n == name && is_super(qual),
        _ => false,
    }
}

fn is_super(tree: &Tree) -> bool {
    matches!(&tree.kind, K::Ident(name) if name == "super")
}
