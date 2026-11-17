//! B001: `equals(Object)` without `hashCode()` or vice versa (Checkstyle EqualsHashCode, PMD
//! OverrideBothEqualsAndHashcode).

use tree_sitter::Node;

use crate::lint::{Diagnostic, Rule, source};

pub fn check(body: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let methods: Vec<Node> = body.named_children(&mut body.walk()).filter(|m| is_implemented_method(m, text)).collect();
    let equals = methods.iter().find(|m| is_equals(m, text));
    let hash_code = methods.iter().find(|m| is_hash_code(m, text));
    let (method, message) = match (equals, hash_code) {
        (Some(equals), None) => (equals, "equals(Object) is overridden without hashCode()"),
        (None, Some(hash_code)) => (hash_code, "hashCode() is overridden without equals(Object)"),
        _ => return,
    };
    let name = method.child_by_field_name("name").unwrap_or(*method);
    diagnostics.push(Diagnostic::new(Rule::EqualsHashCode, name.start_byte(), message));
}

/// Abstract declarations do not implement the contract, so they count as missing.
fn is_implemented_method(node: &Node, text: &str) -> bool {
    node.kind() == "method_declaration" && (node.child_by_field_name("body").is_some() || is_native(node, text))
}

fn is_native(method: &Node, text: &str) -> bool {
    let modifiers = method.named_children(&mut method.walk()).find(|c| c.kind() == "modifiers");
    modifiers.is_some_and(|m| m.children(&mut m.walk()).any(|c| source(&c, text) == "native"))
}

fn is_equals(method: &Node, text: &str) -> bool {
    let parameters = parameter_types(method, text);
    has_name(method, text, "equals") && matches!(parameters.as_slice(), ["Object" | "java.lang.Object"])
}

fn is_hash_code(method: &Node, text: &str) -> bool {
    has_name(method, text, "hashCode") && parameter_types(method, text).is_empty()
}

fn has_name(method: &Node, text: &str, name: &str) -> bool {
    method.child_by_field_name("name").is_some_and(|n| source(&n, text) == name)
}

fn parameter_types<'a>(method: &Node, text: &'a str) -> Vec<&'a str> {
    let Some(parameters) = method.child_by_field_name("parameters") else { return Vec::new() };
    parameters
        .named_children(&mut parameters.walk())
        .filter_map(|p| p.child_by_field_name("type"))
        .map(|t| source(&t, text))
        .collect()
}
