//! B028: a parameter of a private method or constructor that its body never reads (PMD UnusedFormalParameter with
//! its default `checkAll=false`).
//!
//! As in PMD, parameters named `_` or starting with `ignored` or `unused` are allowed, and methods without a body,
//! `default` methods and the serialization method `readObject(ObjectInputStream) throws InvalidObjectException` are
//! skipped (PMD's exact condition). tree-sitter parses an annotated receiver parameter (`@A Foo this`) as a parameter
//! named `this`, which is not a parameter. Assigning a parameter, or incrementing it in a statement of its own, is not
//! reading it. Uses are found by name, without PMD's symbol table: a local variable or lambda parameter of the same
//! name that shadows the parameter counts as a use of it.

use tree_sitter::Node;

use crate::lint::syntax::{has_modifier, is_name_reference_in, named_children, operator, parameter_name, source};
use crate::lint::{Diagnostic, Rule};

pub fn check(declaration: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if !has_modifier(declaration, text, "private") || has_modifier(declaration, text, "default") {
        return;
    }
    let Some(body) = declaration.child_by_field_name("body") else { return };
    let Some(parameters) = declaration.child_by_field_name("parameters") else { return };
    let names: Vec<Node> = named_children(&parameters)
        .iter()
        .filter(|p| matches!(p.kind(), "formal_parameter" | "spread_parameter"))
        .filter_map(parameter_name)
        .filter(|n| {
            let name = source(n, text);
            name != "_" && name != "this" && !name.starts_with("ignored") && !name.starts_with("unused")
        })
        .collect();
    if names.is_empty() || is_serialization_read_object(declaration, &parameters, text) {
        return;
    }
    let read = read_names(&body, text);
    let kind = if declaration.kind() == "method_declaration" { "method" } else { "constructor" };
    for name in names {
        let name_text = source(&name, text);
        if !read.contains(&name_text) {
            let message = format!("Avoid unused {kind} parameters such as '{name_text}'.");
            diagnostics.push(Diagnostic::new(Rule::UnusedParameter, name.start_byte(), message));
        }
    }
}

/// `private void readObject(ObjectInputStream in) throws InvalidObjectException`, which PMD skips.
fn is_serialization_read_object(declaration: &Node, parameters: &Node, text: &str) -> bool {
    let is_type = |node: &Node, simple: &str, qualified: &str| {
        let name: String = source(node, text).split_whitespace().collect();
        name == simple || name == qualified
    };
    let name_is_read_object = declaration.child_by_field_name("name").is_some_and(|n| source(&n, text) == "readObject");
    let throws: Vec<Node> = named_children(declaration)
        .into_iter()
        .find(|c| c.kind() == "throws")
        .map(|t| named_children(&t).into_iter().filter(|c| !c.is_extra()).collect())
        .unwrap_or_default();
    let formals: Vec<Node> = named_children(parameters).into_iter().filter(|c| !c.is_extra()).collect();
    name_is_read_object
        && matches!(throws.as_slice(), [t] if is_type(t, "InvalidObjectException", "java.io.InvalidObjectException"))
        && matches!(formals.as_slice(), [p] if p.kind() == "formal_parameter"
            && p.child_by_field_name("type").is_some_and(|t| is_type(&t, "ObjectInputStream", "java.io.ObjectInputStream")))
}

/// The names the body reads, as PMD's read usages: a name that is not only assigned, nor incremented or compound
/// assigned in a statement of its own.
fn read_names<'a>(body: &Node, text: &'a str) -> Vec<&'a str> {
    let mut read = Vec::new();
    let mut cursor = body.walk();
    // The ancestors of the cursor's node within the body, the parent last.
    let mut ancestors: Vec<Node> = vec![*body];
    if !cursor.goto_first_child() {
        return read;
    }
    loop {
        let node = cursor.node();
        if node.kind() == "identifier" {
            let parent = ancestors[ancestors.len() - 1];
            let grandparent = ancestors.len().checked_sub(2).map(|i| ancestors[i]);
            if is_name_reference_in(&node, &parent) && is_read(&node, &parent, grandparent.as_ref(), text) {
                read.push(source(&node, text));
            }
        }
        if cursor.goto_first_child() {
            ancestors.push(node);
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() || ancestors.len() == 1 {
                return read;
            }
            ancestors.pop();
        }
    }
}

fn is_read(identifier: &Node, parent: &Node, grandparent: Option<&Node>, text: &str) -> bool {
    let is_statement = || grandparent.is_some_and(|g| g.kind() == "expression_statement");
    match parent.kind() {
        "assignment_expression" if parent.child_by_field_name("left") == Some(*identifier) => {
            operator(parent, text) != "=" && !is_statement()
        }
        "update_expression" => !is_statement(),
        _ => true,
    }
}
