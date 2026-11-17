//! B026: local variable that is declared, and maybe assigned, but never read (Checkstyle UnusedLocalVariable, PMD
//! UnusedLocalVariable, Error Prone UnusedVariable). Covers `for` variables and pattern variables; `_` is unnamed.
//!
//! Unsafe fix (Eclipse save action "Remove unused local variables"): removes the declaration of a variable that is
//! never assigned afterwards, if its value has no side effects ([`is_side_effect_free`]); from a declaration of several
//! variables only its declarator. Unsafe because a name in the value could be a field whose access loads a class.
//! Eclipse also removes assignments; jtk leaves such variables.

use tree_sitter::Node;

use crate::lint::syntax::{
    has_comment, is_name_reference, is_only_written, is_side_effect_free, line_removal_range, named_children, operator,
    source, strip_parentheses,
};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(node: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    for (name, scope) in declared_variables(node, text) {
        let variable = source(&name, text);
        if variable == "_" {
            continue;
        }
        let references = references(variable, &scope, name.end_byte(), text);
        if references.read {
            continue;
        }
        let message = format!("Unused local variable '{variable}'");
        let diagnostic = Diagnostic::new(Rule::UnusedLocalVariable, name.start_byte(), message);
        let fix = if references.written { None } else { removal(node, &name, text) };
        diagnostics.push(match fix {
            Some(edit) => diagnostic.with_unsafe_fix(vec![edit]),
            None => diagnostic,
        });
    }
}

/// The edit that removes the declaration of the variable `name` from a local variable declaration statement, if its
/// value has no side effects.
fn removal(declaration: &Node, name: &Node, text: &str) -> Option<Edit> {
    if declaration.kind() != "local_variable_declaration" || has_comment(declaration) {
        return None;
    }
    let in_statement_list = declaration
        .parent()
        .is_some_and(|p| matches!(p.kind(), "block" | "constructor_body" | "switch_block_statement_group"));
    let declarators: Vec<Node> =
        named_children(declaration).into_iter().filter(|d| d.kind() == "variable_declarator").collect();
    let index = declarators.iter().position(|d| d.child_by_field_name("name") == Some(*name))?;
    let declarator = declarators[index];
    let value_is_pure = declarator.child_by_field_name("value").is_none_or(|v| is_side_effect_free(&v));
    if !in_statement_list || !value_is_pure {
        return None;
    }
    let range = match (index.checked_sub(1).map(|i| declarators[i]), declarators.get(index + 1)) {
        (_, Some(next)) => declarator.start_byte()..next.start_byte(),
        (Some(previous), None) => previous.end_byte()..declarator.end_byte(),
        (None, None) => line_removal_range(declaration, text),
    };
    Some(Edit::new(range, ""))
}

/// The names the node declares, each with the node it is visible in.
fn declared_variables<'t>(node: &Node<'t>, text: &str) -> Vec<(Node<'t>, Node<'t>)> {
    let Some(parent) = node.parent() else { return Vec::new() };
    match node.kind() {
        "local_variable_declaration" => {
            // A variable declared in a case is visible in the following cases too.
            let scope = if parent.kind() == "switch_block_statement_group" {
                parent.parent().unwrap_or(parent)
            } else {
                parent
            };
            let declarators = named_children(node).into_iter().filter(|d| d.kind() == "variable_declarator");
            declarators.filter_map(|d| d.child_by_field_name("name")).map(|n| (n, scope)).collect()
        }
        "enhanced_for_statement" => node.child_by_field_name("name").map(|n| (n, *node)).into_iter().collect(),
        "instanceof_expression" => {
            node.child_by_field_name("name").map(|n| (n, pattern_scope(node, text))).into_iter().collect()
        }
        "type_pattern" | "record_pattern_component" => {
            let name = named_children(node).into_iter().last().filter(|n| n.kind() == "identifier");
            name.map(|n| (n, pattern_scope(node, text))).into_iter().collect()
        }
        _ => Vec::new(),
    }
}

/// Where a pattern variable is visible: its `case`, or the statement with the `instanceof`. After an
/// `if (!(o instanceof T t))`, `t` stays visible in the rest of the block.
fn pattern_scope<'t>(pattern: &Node<'t>, text: &str) -> Node<'t> {
    let mut ancestors = std::iter::successors(pattern.parent(), |n| n.parent());
    let Some(statement) = ancestors.find(|n| {
        matches!(n.kind(), "switch_rule" | "switch_block_statement_group" | "lambda_expression")
            || n.parent()
                .is_some_and(|p| matches!(p.kind(), "block" | "constructor_body" | "switch_block_statement_group"))
    }) else {
        return *pattern;
    };
    let in_negated_condition = statement.kind() == "if_statement"
        && statement.child_by_field_name("condition").is_some_and(|c| {
            let condition = strip_parentheses(&c);
            condition.kind() == "unary_expression" && operator(&condition, text) == "!"
        });
    match statement.parent() {
        Some(block) if in_negated_condition => block,
        _ => statement,
    }
}

#[derive(Default)]
struct References {
    read: bool,
    written: bool,
}

/// Whether the variable is read, and whether it is written, after `after` in its scope. Code in a nested class that
/// declares the same name refers to its own variable.
fn references(variable: &str, scope: &Node, after: usize, text: &str) -> References {
    let mut references = References::default();
    let mut pending = named_children(scope);
    while let Some(node) = pending.pop() {
        if node.kind() == "identifier"
            && node.start_byte() >= after
            && source(&node, text) == variable
            && is_name_reference(&node)
        {
            if is_only_written(&node, text) {
                references.written = true;
            } else {
                references.read = true;
                return references;
            }
        }
        if node.kind() != "class_body" || !declares(&node, variable, text) {
            pending.extend(named_children(&node));
        }
    }
    references
}

fn declares(class_body: &Node, variable: &str, text: &str) -> bool {
    let mut pending = named_children(class_body);
    while let Some(node) = pending.pop() {
        let name = match node.kind() {
            "variable_declarator" | "formal_parameter" => node.child_by_field_name("name"),
            _ => None,
        };
        if name.is_some_and(|n| source(&n, text) == variable) {
            return true;
        }
        pending.extend(named_children(&node));
    }
    false
}
