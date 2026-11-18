//! B002: `equals(Foo)` without `equals(Object)` (Checkstyle CovariantEquals): the covariant method does not override
//! `Object.equals`, so collections ignore it.

use super::shared_a::{class_bodies, is_varargs};
use crate::ast::{K, MethodDecl, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    for members in class_bodies(tree) {
        let equals_methods = || {
            members.iter().filter_map(|m| match &m.kind {
                K::Method(method) if method.ret.is_some() && method.name == "equals" => {
                    single_parameter_type(method, context.text).map(|t| (&**method, t))
                }
                _ => None,
            })
        };
        if equals_methods().any(|(_, t)| matches!(t, "Object" | "java.lang.Object")) {
            continue;
        }
        // An abstract covariant equals is a declaration for subclasses, not the mistaken implementation.
        for (method, _) in equals_methods().filter(|(m, _)| m.body.is_some()) {
            let message = "Covariant equals without equals(Object)";
            diagnostics.push(Diagnostic::new(Rule::CovariantEquals, method.name_pos, message));
        }
    }
}

fn single_parameter_type<'a>(method: &MethodDecl, text: &'a str) -> Option<&'a str> {
    match method.params.as_slice() {
        [parameter] if method.receiver.is_none() && !is_varargs(parameter, text) => {
            parameter.ty.as_deref().map(|t| &text[t.start..t.end])
        }
        _ => None,
    }
}
