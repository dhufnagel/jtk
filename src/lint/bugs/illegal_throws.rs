//! B021: declaring `throws RuntimeException`, `Error` or `Throwable` (Checkstyle IllegalThrows with its default
//! properties, PMD AvoidThrowingRawExceptionTypes): it tells callers nothing about what can fail.
//!
//! As in Checkstyle, `finalize` and methods annotated with `@Override` (whose signature the overridden method
//! dictates) are not checked.

use super::illegal_catch::illegal_name;
use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    let text = context.text;
    let has_override = || {
        method.mods.annotations.iter().any(|a| {
            matches!(&a.kind, K::Annotation { ty, .. } if matches!(&text[ty.start..ty.end], "Override" | "java.lang.Override"))
        })
    };
    if method.throws.is_empty() || method.name == "finalize" || has_override() {
        return;
    }
    for ty in &method.throws {
        if let Some(name) = illegal_name(ty, text, &context.options.illegal_throws.illegal_class_names) {
            // Checkstyle reports a qualified name at its last dot.
            let dot = match &ty.kind {
                K::MemberSelect { .. } => {
                    context.tokens().in_range(ty.start..ty.end).filter(|t| &text[t.clone()] == ".").last()
                }
                _ => None,
            };
            let offset = dot.map_or(ty.start, |d| d.start);
            diagnostics.push(Diagnostic::new(Rule::IllegalThrows, offset, format!("Throwing '{name}' is not allowed")));
        }
    }
}
