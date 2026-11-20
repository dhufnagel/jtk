//! A004: a non-final field in an exception class (Checkstyle MutableException with its default formats: the class
//! name and the name of the class it extends match `^.*Exception$|^.*Error$|^.*Throwable$`).
//!
//! Like Checkstyle, the fields of the types nested in the exception class are checked too (except those of nested
//! classes, which are checked on their own), and an extended class with type arguments does not count as an
//! exception.

use super::has_keyword;
use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    if class.kind != ClassKind::Class || !is_exception_name(&class.name) || !extends_exception(class.extends.as_deref())
    {
        return;
    }
    let is_class = |node: &Tree| matches!(&node.kind, K::Class(nested) if nested.kind == ClassKind::Class);
    for member in class.members.iter().filter(|m| !is_class(m)) {
        check_field(member, tree, context, diagnostics);
        for_each_descendant(member, &mut |node, parent| {
            check_field(node, parent, context, diagnostics);
            !is_class(node)
        });
    }
}

/// Reports a field declaration that is not final.
fn check_field(node: &Tree, parent: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Vars(vars) = &node.kind else { return };
    let is_field = matches!(parent.kind, K::Class(_) | K::NewClass { .. });
    if is_field && !has_keyword(&vars[0].mods, vars[0].name_pos, "final", context) {
        for var in vars {
            let message = format!("The field '{}' must be declared final", var.name);
            diagnostics.push(Diagnostic::new(Rule::MutableException, node.start, message));
        }
    }
}

fn is_exception_name(name: &str) -> bool {
    name.ends_with("Exception") || name.ends_with("Error") || name.ends_with("Throwable")
}

/// Whether the class extends a class named like an exception, without type arguments.
fn extends_exception(superclass: Option<&Tree>) -> bool {
    match superclass.map(|s| &s.kind) {
        Some(K::Ident(name) | K::MemberSelect { name, .. }) => is_exception_name(name),
        _ => false,
    }
}
