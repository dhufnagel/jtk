//! B001: `equals(Object)` without `hashCode()` or vice versa (Checkstyle EqualsHashCode, PMD
//! OverrideBothEqualsAndHashcode).

use super::shared_a::{class_bodies, is_varargs};
use crate::lint::tree::{Context, has_modifier};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, MethodDecl, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    for members in class_bodies(tree) {
        let implemented = || {
            members.iter().filter_map(|m| match &m.kind {
                K::Method(method) if is_implemented(method, context) => Some(&**method),
                _ => None,
            })
        };
        let equals = implemented().find(|m| is_equals(m, context.text));
        let hash_code = implemented().find(|m| is_hash_code(m, context.text));
        let (method, message) = match (equals, hash_code) {
            (Some(equals), None) => (equals, "equals(Object) is overridden without hashCode()"),
            (None, Some(hash_code)) => (hash_code, "hashCode() is overridden without equals(Object)"),
            _ => continue,
        };
        diagnostics.push(Diagnostic::new(Rule::EqualsHashCode, method.name_pos, message));
    }
}

/// Abstract declarations do not implement the contract, so they count as missing.
fn is_implemented(method: &MethodDecl, context: &Context) -> bool {
    method.ret.is_some()
        && (method.body.is_some() || has_modifier(&method.mods, context.text, context.tokens(), "native"))
}

fn is_equals(method: &MethodDecl, text: &str) -> bool {
    method.name == "equals" && matches!(parameter_types(method, text).as_slice(), ["Object" | "java.lang.Object"])
}

fn is_hash_code(method: &MethodDecl, text: &str) -> bool {
    method.name == "hashCode" && parameter_types(method, text).is_empty()
}

/// The types of the parameters, as written; like Checkstyle, without a variable arity parameter.
fn parameter_types<'a>(method: &MethodDecl, text: &'a str) -> Vec<&'a str> {
    let fixed = method.params.iter().filter(|p| !is_varargs(p, text));
    fixed.filter_map(|p| p.ty.as_deref()).map(|t| &text[t.start..t.end]).collect()
}
