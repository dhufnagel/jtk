//! A004: a non-final field in an exception class (Checkstyle MutableException with its default formats: the class
//! name and the name of the class it extends match `^.*Exception$|^.*Error$|^.*Throwable$`).
//!
//! Like Checkstyle, the fields of the types nested in the exception class are checked too (except those of nested
//! classes, which are checked on their own), and an extended class with type arguments does not count as an
//! exception.

use tree_sitter::Node;

use super::{body, declarator_names, has_keyword};
use crate::lint::syntax::{for_each_descendant, named_children, source};
use crate::lint::{Diagnostic, Rule};

pub fn check(class: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let name = class.child_by_field_name("name").map_or("", |n| source(&n, text));
    if !is_exception_name(name) || !extends_exception(class, text) {
        return;
    }
    let Some(body) = body(class) else { return };
    check_fields(&body, text, diagnostics);
}

/// Reports the non-final fields in a body, skipping nested classes.
fn check_fields(body: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    for_each_descendant(body, &["class_declaration"], |node| {
        if matches!(node.kind(), "field_declaration" | "constant_declaration") && !has_keyword(node, text, "final") {
            for name in declarator_names(node) {
                let message = format!("The field '{}' must be declared final", source(&name, text));
                diagnostics.push(Diagnostic::new(Rule::MutableException, node.start_byte(), message));
            }
        }
    });
}

fn is_exception_name(name: &str) -> bool {
    name.ends_with("Exception") || name.ends_with("Error") || name.ends_with("Throwable")
}

/// Whether the class extends a class named like an exception, without type arguments.
fn extends_exception(class: &Node, text: &str) -> bool {
    let Some(superclass) = class.child_by_field_name("superclass") else { return false };
    let Some(ty) = named_children(&superclass).into_iter().last() else { return false };
    let last_name = match ty.kind() {
        "type_identifier" => source(&ty, text),
        "scoped_type_identifier" => named_children(&ty).last().map_or("", |n| source(n, text)),
        _ => return false,
    };
    is_exception_name(last_name)
}
