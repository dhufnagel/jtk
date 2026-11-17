//! B033 and B034: declaring `finalize()` (Checkstyle NoFinalizer) or `clone()` (Checkstyle NoClone). Finalizers are
//! deprecated and run unpredictably; `clone` is hard to implement correctly, a copy constructor or factory is safer.
//!
//! As in Checkstyle, only the overloads without parameters count, in any type including interfaces, and the
//! violation is reported where the declaration starts (at its annotations, if any).

use tree_sitter::Node;

use crate::lint::syntax::{is_comment, source};
use crate::lint::{Diagnostic, Rule};

pub fn check_finalizer(method: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if is_parameterless(method, text, "finalize") {
        diagnostics.push(Diagnostic::new(Rule::Finalizer, method.start_byte(), "Avoid using finalizer method"));
    }
}

pub fn check_clone(method: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if is_parameterless(method, text, "clone") {
        diagnostics.push(Diagnostic::new(Rule::Clone, method.start_byte(), "Avoid using clone method"));
    }
}

/// Whether the method has the name and no parameters (a receiver parameter counts as one, as in Checkstyle).
pub(super) fn is_parameterless(method: &Node, text: &str, name: &str) -> bool {
    method.child_by_field_name("name").is_some_and(|n| source(&n, text) == name)
        && method
            .child_by_field_name("parameters")
            .is_some_and(|p| !p.named_children(&mut p.walk()).any(|c| !is_comment(&c)))
}
