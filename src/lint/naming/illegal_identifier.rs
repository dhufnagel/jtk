//! N014: a declared name that is `var` or contains `$` (Checkstyle IllegalIdentifierName with its default format
//! `^(?!var$|\S*\$)\S+$`). Checks the names of types, methods, annotation elements, enum constants, fields,
//! variables, parameters (also of lambdas), record components and pattern variables.

use tree_sitter::Node;

use super::super::syntax::{named_children, source};
use super::super::{Diagnostic, Rule};
use super::parameter_name_node;

const PATTERN: &str = r"^(?!var$|\S*\$)\S+$";

pub fn check(node: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    for name in names(node) {
        let value = source(&name, text);
        if value == "var" || value.contains('$') {
            let message = format!("Name '{value}' must match pattern '{PATTERN}'");
            diagnostics.push(Diagnostic::new(Rule::IllegalIdentifier, name.start_byte(), message));
        }
    }
}

/// The names a node declares.
fn names<'t>(node: &Node<'t>) -> Vec<Node<'t>> {
    match node.kind() {
        "field_declaration" | "local_variable_declaration" | "constant_declaration" => named_children(node)
            .into_iter()
            .filter(|d| d.kind() == "variable_declarator")
            .filter_map(|d| d.child_by_field_name("name"))
            .collect(),
        "formal_parameter" | "spread_parameter" | "catch_formal_parameter" => {
            parameter_name_node(node).into_iter().collect()
        }
        "inferred_parameters" => named_children(node).into_iter().filter(|c| c.kind() == "identifier").collect(),
        "lambda_expression" => {
            node.child_by_field_name("parameters").filter(|p| p.kind() == "identifier").into_iter().collect()
        }
        "type_pattern" | "record_pattern_component" => {
            named_children(node).into_iter().last().filter(|n| n.kind() == "identifier").into_iter().collect()
        }
        _ => node.child_by_field_name("name").filter(|n| n.kind() == "identifier").into_iter().collect(),
    }
}
