//! B013: assigning to a parameter, also with `+=` or `++` (Checkstyle ParameterAssignment, PMD
//! AvoidReassigningParameters): the parameter no longer holds the argument, which confuses readers.
//!
//! As in Checkstyle, the parameters of the innermost method, constructor or lambda count, also in the initializers of
//! local and anonymous classes in it; the components of a record are not parameters of its compact constructor.

use super::shared_a::modified_name;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree, VarDecl};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (parameters, body): (&[VarDecl], &Tree) = match &tree.kind {
        K::Method(method) => match &method.body {
            Some(body) if !method.compact_ctor => (&method.params, body),
            _ => return,
        },
        K::Lambda { params, body } => (params, body),
        _ => return,
    };
    if parameters.is_empty() {
        return;
    }
    let mut check_tree = |tree: &Tree| {
        let Some(name) = modified_name(tree) else { return };
        if !parameters.iter().any(|p| p.name == name) {
            return;
        }
        let offset = match &tree.kind {
            K::Assign { lhs, rhs, .. } => context.tokens().in_range(lhs.end..rhs.start).next(),
            K::Unary { postfix: true, .. } => context.tokens().previous(tree.end),
            _ => None,
        };
        let message = format!("Assignment of parameter '{name}' is not allowed");
        diagnostics.push(Diagnostic::new(Rule::ParameterAssignment, offset.map_or(tree.start, |o| o.start), message));
    };
    check_tree(body);
    for_each_in_scope(body, &mut check_tree);
}

/// Calls `f` on each descendant outside nested methods, constructors, lambdas and annotations.
fn for_each_in_scope<'t>(tree: &'t Tree, f: &mut impl FnMut(&'t Tree)) {
    tree.for_each_child(&mut |child| match child.kind {
        K::Method(_) | K::Lambda { .. } | K::Annotation { .. } => {}
        _ => {
            f(child);
            for_each_in_scope(child, f);
        }
    });
}
