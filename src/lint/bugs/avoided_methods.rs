//! B033 and B034: declaring `finalize()` (Checkstyle NoFinalizer) or `clone()` (Checkstyle NoClone). Finalizers are
//! deprecated and run unpredictably; `clone` is hard to implement correctly, a copy constructor or factory is safer.
//!
//! As in Checkstyle, only the overloads without parameters count, in any type including interfaces, and the
//! violation is reported where the declaration starts (at its annotations, if any).

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, MethodDecl, Tree};

pub fn check_finalizer(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if declared_method(tree, context).is_some_and(|m| is_parameterless(m, "finalize")) {
        diagnostics.push(Diagnostic::new(Rule::Finalizer, tree.start, "Avoid using finalizer method"));
    }
}

pub fn check_clone(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if declared_method(tree, context).is_some_and(|m| is_parameterless(m, "clone")) {
        diagnostics.push(Diagnostic::new(Rule::Clone, tree.start, "Avoid using clone method"));
    }
}

/// The method declared by the tree: not a constructor nor an element of an annotation type.
pub(super) fn declared_method<'t>(tree: &'t Tree, context: &Context) -> Option<&'t MethodDecl> {
    let K::Method(method) = &tree.kind else { return None };
    let in_annotation_type =
        context.parent().is_some_and(|p| matches!(&p.kind, K::Class(c) if c.kind == ClassKind::Annotation));
    (method.ret.is_some() && !in_annotation_type).then_some(method)
}

/// Whether the method has the name and no parameters (a receiver parameter counts as one, as in Checkstyle).
pub(super) fn is_parameterless(method: &MethodDecl, name: &str) -> bool {
    method.name == name && method.params.is_empty() && method.receiver.is_none()
}
