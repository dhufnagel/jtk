//! B021: declaring `throws RuntimeException`, `Error` or `Throwable` (Checkstyle IllegalThrows with its default
//! properties, PMD AvoidThrowingRawExceptionTypes): it tells callers nothing about what can fail.
//!
//! As in Checkstyle, `finalize` and methods annotated with `@Override` (whose signature the overridden method
//! dictates) are not checked.

use tree_sitter::Node;

use super::illegal_catch::illegal_name;
use crate::lint::syntax::{is_comment, modifiers, named_children, source};
use crate::lint::{Diagnostic, Rule, RuleOptions};

pub fn check(declaration: &Node, text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let Some(throws) = named_children(declaration).into_iter().find(|c| c.kind() == "throws") else { return };
    let is_finalize = declaration.child_by_field_name("name").is_some_and(|n| source(&n, text) == "finalize");
    if is_finalize || has_override(declaration, text) {
        return;
    }
    for type_node in named_children(&throws).into_iter().filter(|c| !is_comment(c)) {
        if let Some(name) = illegal_name(&type_node, text, &options.illegal_throws.illegal_class_names) {
            // Checkstyle reports a qualified name at its last dot.
            let dot = type_node.children(&mut type_node.walk()).filter(|c| c.kind() == ".").last();
            let offset = dot.map_or(type_node.start_byte(), |d| d.start_byte());
            diagnostics.push(Diagnostic::new(Rule::IllegalThrows, offset, format!("Throwing '{name}' is not allowed")));
        }
    }
}

fn has_override(declaration: &Node, text: &str) -> bool {
    modifiers(declaration).is_some_and(|m| {
        named_children(&m).iter().any(|a| {
            matches!(a.kind(), "marker_annotation" | "annotation")
                && a.child_by_field_name("name")
                    .is_some_and(|n| matches!(source(&n, text), "Override" | "java.lang.Override"))
        })
    })
}
