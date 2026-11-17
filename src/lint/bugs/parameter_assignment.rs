//! B013: assigning to a parameter, also with `+=` or `++` (Checkstyle ParameterAssignment, PMD
//! AvoidReassigningParameters): the parameter no longer holds the argument, which confuses readers.
//!
//! As in Checkstyle, the parameters of the innermost method, constructor or lambda count, also in the initializers of
//! local and anonymous classes in it; the components of a record are not parameters of its compact constructor.

use std::collections::HashSet;

use tree_sitter::Node;

use crate::lint::syntax::{for_each_descendant, named_children, source};
use crate::lint::{Diagnostic, Rule};

/// The scopes with their own parameters, checked separately.
const SCOPES: &[&str] = &["method_declaration", "constructor_declaration", "lambda_expression"];

pub fn check(scope: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(parameters) = scope.child_by_field_name("parameters") else { return };
    let names = parameter_names(&parameters, text);
    let Some(body) = scope.child_by_field_name("body") else { return };
    if names.is_empty() {
        return;
    }
    let mut check_node = |node: &Node| {
        let (target, operator) = match node.kind() {
            "assignment_expression" => (node.child_by_field_name("left"), node.child_by_field_name("operator")),
            "update_expression" => {
                let children: Vec<Node> = node.children(&mut node.walk()).collect();
                (
                    children.iter().find(|c| c.is_named()).copied(),
                    children.iter().find(|c| matches!(c.kind(), "++" | "--")).copied(),
                )
            }
            _ => return,
        };
        let (Some(target), Some(operator)) = (target, operator) else { return };
        if target.kind() == "identifier" && names.contains(source(&target, text)) {
            let message = format!("Assignment of parameter '{}' is not allowed", source(&target, text));
            diagnostics.push(Diagnostic::new(Rule::ParameterAssignment, operator.start_byte(), message));
        }
    };
    check_node(&body);
    for_each_descendant(&body, SCOPES, check_node);
}

fn parameter_names<'a>(parameters: &Node, text: &'a str) -> HashSet<&'a str> {
    if parameters.kind() == "identifier" {
        return HashSet::from([source(parameters, text)]);
    }
    let mut names = HashSet::new();
    for parameter in named_children(parameters) {
        let name = match parameter.kind() {
            "identifier" => Some(parameter),
            "formal_parameter" => parameter.child_by_field_name("name"),
            "spread_parameter" => named_children(&parameter)
                .into_iter()
                .find(|c| c.kind() == "variable_declarator")
                .and_then(|d| d.child_by_field_name("name")),
            _ => None,
        };
        names.extend(name.map(|n| source(&n, text)));
    }
    names
}
