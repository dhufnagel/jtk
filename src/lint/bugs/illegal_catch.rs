//! B020: catching `Exception`, `RuntimeException`, `Error` or `Throwable` (Checkstyle IllegalCatch with its default
//! properties, PMD AvoidCatchingThrowable and AvoidCatchingGenericException): it also catches unexpected failures,
//! which then go unnoticed.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

/// The type's name if it is one of `illegal` or the same in `java.lang`.
pub fn illegal_name(ty: &Tree, text: &str, illegal: &[String]) -> Option<String> {
    let name: String = text[ty.start..ty.end].split_whitespace().collect();
    let simple = name.strip_prefix("java.lang.").unwrap_or(&name);
    illegal.iter().any(|i| i == simple).then_some(name)
}

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Try { catches, .. } = &tree.kind else { return };
    let illegal = &context.options.illegal_catch.illegal_class_names;
    for catch in catches {
        let single = catch.param.ty.as_deref().filter(|_| catch.union.is_empty());
        for ty in catch.union.iter().chain(single) {
            if let Some(name) = illegal_name(ty, context.text, illegal) {
                let message = format!("Catching '{name}' is not allowed");
                diagnostics.push(Diagnostic::new(Rule::IllegalCatch, catch.start, message));
            }
        }
    }
}
