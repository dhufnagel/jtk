//! B014: `for` loop variable modified in the loop body (Checkstyle ModifiedControlVariable, PMD
//! AvoidReassigningLoopVariables).

use tree_sitter::Node;

use crate::lint::syntax::{NESTED_CODE, for_each_descendant, named_children, source};
use crate::lint::{Diagnostic, Rule};

pub fn check(statement: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let variables = control_variables(statement, text);
    let Some(body) = statement.child_by_field_name("body") else { return };
    if variables.is_empty() {
        return;
    }
    for_each_descendant(&body, NESTED_CODE, |node| {
        if let Some(target) = modified_variable(node)
            && variables.contains(&source(&target, text))
        {
            let message = format!("Loop variable '{}' is modified in the loop body", source(&target, text));
            diagnostics.push(Diagnostic::new(Rule::ModifiedControlVariable, node.start_byte(), message));
        }
    });
}

/// The loop's control variables: declared in the `for` initializer and updated by the loop, or the enhanced `for`
/// variable. Like Checkstyle, a variable only updated in the body is not a control variable.
fn control_variables<'a>(statement: &Node, text: &'a str) -> Vec<&'a str> {
    if statement.kind() == "enhanced_for_statement" {
        return loop_variables(statement, text);
    }
    let updated = updated_variables(statement, text);
    loop_variables(statement, text).into_iter().filter(|v| updated.contains(v)).collect()
}

/// The variables written by the `for` update expressions.
pub fn updated_variables<'a>(statement: &Node, text: &'a str) -> Vec<&'a str> {
    let mut cursor = statement.walk();
    let updates: Vec<Node> = statement.children_by_field_name("update", &mut cursor).collect();
    updates.iter().filter_map(modified_variable).map(|v| source(&v, text)).collect()
}

/// The variables declared by the loop: in the `for` initializer, or the enhanced `for` variable.
pub fn loop_variables<'a>(statement: &Node, text: &'a str) -> Vec<&'a str> {
    if statement.kind() == "enhanced_for_statement" {
        return statement.child_by_field_name("name").map(|n| source(&n, text)).into_iter().collect();
    }
    let mut cursor = statement.walk();
    let initializers: Vec<Node> = statement.children_by_field_name("init", &mut cursor).collect();
    initializers
        .iter()
        .filter(|i| i.kind() == "local_variable_declaration")
        .flat_map(named_children)
        .filter(|d| d.kind() == "variable_declarator")
        .filter_map(|d| d.child_by_field_name("name"))
        .map(|n| source(&n, text))
        .collect()
}

/// The variable an assignment or increment writes to, if it is a plain name.
pub fn modified_variable<'t>(node: &Node<'t>) -> Option<Node<'t>> {
    let target = match node.kind() {
        "assignment_expression" => node.child_by_field_name("left")?,
        "update_expression" => node.named_child(0)?,
        _ => return None,
    };
    (target.kind() == "identifier").then_some(target)
}
