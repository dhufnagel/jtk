//! B020: catching `Exception`, `RuntimeException`, `Error` or `Throwable` (Checkstyle IllegalCatch with its default
//! properties, PMD AvoidCatchingThrowable and AvoidCatchingGenericException): it also catches unexpected failures,
//! which then go unnoticed.

use tree_sitter::Node;

use crate::lint::syntax::{is_comment, named_children, source};
use crate::lint::{Diagnostic, Rule, RuleOptions};

/// The type's name if it is one of `illegal` or the same in `java.lang`.
pub fn illegal_name(type_node: &Node, text: &str, illegal: &[String]) -> Option<String> {
    let name: String = source(type_node, text).split_whitespace().collect();
    let simple = name.strip_prefix("java.lang.").unwrap_or(&name);
    illegal.iter().any(|i| i == simple).then_some(name)
}

pub fn check(catch: &Node, text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let parameter = named_children(catch).into_iter().find(|c| c.kind() == "catch_formal_parameter");
    let Some(catch_type) = parameter.and_then(|p| named_children(&p).into_iter().find(|c| c.kind() == "catch_type"))
    else {
        return;
    };
    for type_node in named_children(&catch_type).into_iter().filter(|c| !is_comment(c)) {
        if let Some(name) = illegal_name(&type_node, text, &options.illegal_catch.illegal_class_names) {
            let message = format!("Catching '{name}' is not allowed");
            diagnostics.push(Diagnostic::new(Rule::IllegalCatch, catch.start_byte(), message));
        }
    }
}
