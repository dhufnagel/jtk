//! N013: more than four consecutive capital letters in a name (Checkstyle AbbreviationAsWordInName, by default with
//! `allowed-abbreviation-length` 3, so `parseXMLDoc` passes but `XMLHTTPRequest` does not).
//!
//! As in Checkstyle, types, methods, annotation elements, fields, local variables, parameters, record components and
//! pattern variables are checked; `static` and `final` fields and variables, interface fields and `@Override`
//! methods are not.

use tree_sitter::Node;

use super::super::syntax::{has_modifier, modifiers, named_children, source};
use super::super::{Diagnostic, Rule, RuleOptions};
use super::parameter_name_node;

pub fn check(node: &Node, text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    // An abbreviation and the capital starting the next word.
    let allowed_capitals = options.abbreviation_in_name.allowed_abbreviation_length as usize + 1;
    for name in names(node, text) {
        let value = source(&name, text);
        if has_long_abbreviation(value, allowed_capitals) {
            let message = format!(
                "Abbreviation in name '{value}' must contain no more than '{allowed_capitals}' consecutive capital letters"
            );
            diagnostics.push(Diagnostic::new(Rule::AbbreviationInName, name.start_byte(), message));
        }
    }
}

/// The checked names a node declares.
fn names<'t>(node: &Node<'t>, text: &str) -> Vec<Node<'t>> {
    let declarators = |declaration: &Node<'t>| -> Vec<Node<'t>> {
        named_children(declaration)
            .into_iter()
            .filter(|d| d.kind() == "variable_declarator")
            .filter_map(|d| d.child_by_field_name("name"))
            .collect()
    };
    let ignored_modifiers = has_modifier(node, text, "static") || has_modifier(node, text, "final");
    match node.kind() {
        "method_declaration" if is_override(node, text) => Vec::new(),
        "field_declaration" | "local_variable_declaration" if !ignored_modifiers => declarators(node),
        "enhanced_for_statement" if !ignored_modifiers => node.child_by_field_name("name").into_iter().collect(),
        "field_declaration" | "local_variable_declaration" | "enhanced_for_statement" => Vec::new(),
        "formal_parameter" | "spread_parameter" | "catch_formal_parameter" => {
            parameter_name_node(node).filter(|n| source(n, text) != "this").into_iter().collect()
        }
        "inferred_parameters" => named_children(node).into_iter().filter(|c| c.kind() == "identifier").collect(),
        "type_pattern" | "record_pattern_component" => {
            named_children(node).into_iter().last().filter(|n| n.kind() == "identifier").into_iter().collect()
        }
        _ => node.child_by_field_name("name").filter(|n| n.kind() == "identifier").into_iter().collect(),
    }
}

/// Checkstyle only recognizes `@Override` by its simple name.
fn is_override(method: &Node, text: &str) -> bool {
    modifiers(method).is_some_and(|m| {
        named_children(&m).iter().any(|a| a.child_by_field_name("name").is_some_and(|n| source(&n, text) == "Override"))
    })
}

/// Whether the name has a run of more than `allowed_capitals` capitals, followed
/// by a lowercase letter (whose capital starts the next word, `XMLHTTPRequest`), by `_` or by nothing.
fn has_long_abbreviation(name: &str, allowed_capitals: usize) -> bool {
    let mut run = 0;
    for c in name.chars() {
        run = if c.is_uppercase() { run + 1 } else { 0 };
        if run > allowed_capitals {
            return true;
        }
    }
    false
}
