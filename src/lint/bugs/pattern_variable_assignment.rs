//! B031: assigning to a pattern variable, `if (o instanceof String s) { s = s.trim(); }` (Checkstyle
//! PatternVariableAssignment): the variable no longer holds the matched value, which readers expect.
//!
//! As in Checkstyle, this covers the variables of `instanceof` patterns (not of `case` patterns), searched in the
//! scope of the nearest enclosing `if`, `else` or `?:`: for an `if`, its condition and then branch plus the
//! expression, `return` and `if` statements directly after it; for an `else`, its body; for `?:`, the whole
//! expression. Assignments (also compound ones, `++` and `--`) to a variable of the same name in that scope are
//! reported, even if the name is shadowed.
//!
//! Deviations: like Checkstyle's current source, `++` and `--` count, which Checkstyle 14.4.0 does not report yet.
//! Checkstyle only finds one assignment per parent node, so of `c ? (s = a) : (s = b)` only the first; jtk reports
//! both.

use tree_sitter::Node;

use crate::lint::syntax::{for_each_descendant, is_comment, named_children, source};
use crate::lint::{Diagnostic, Rule};

/// The nodes that start a scope of their own.
const SCOPES: &[&str] = &["if_statement", "ternary_expression"];

pub fn check(node: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "ternary_expression" {
        check_scope(&named_children(node), &[*node], text, diagnostics);
        return;
    }
    let condition = node.child_by_field_name("condition");
    let consequence = node.child_by_field_name("consequence");
    let then_scope: Vec<Node> = condition.into_iter().chain(consequence).collect();
    let names = pattern_variables(&then_scope, text);
    if !names.is_empty() {
        let mut branches: Vec<Node> = consequence.into_iter().collect();
        branches.extend(following_statements(node));
        report_assignments(&branches, &names, text, diagnostics);
    }
    if let Some(alternative) = node.child_by_field_name("alternative") {
        check_scope(&[alternative], &[alternative], text, diagnostics);
    }
}

fn check_scope(scope: &[Node], branches: &[Node], text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let names = pattern_variables(scope, text);
    if !names.is_empty() {
        report_assignments(branches, &names, text, diagnostics);
    }
}

/// The names of the variables of the `instanceof` patterns in the nodes (the parts of a scope), outside nested
/// scopes.
fn pattern_variables<'a>(nodes: &[Node], text: &'a str) -> Vec<&'a str> {
    let mut names = Vec::new();
    for node in nodes {
        if SCOPES.contains(&node.kind()) {
            continue;
        }
        let mut visit = |n: &Node| {
            if n.kind() == "instanceof_expression" {
                if let Some(name) = n.child_by_field_name("name") {
                    names.push(source(&name, text));
                } else if let Some(pattern) = n.child_by_field_name("pattern") {
                    record_pattern_variables(&pattern, text, &mut names);
                }
            }
        };
        visit(node);
        for_each_descendant(node, SCOPES, |n| {
            if !SCOPES.contains(&n.kind()) {
                visit(n);
            }
        });
    }
    names
}

fn record_pattern_variables<'a>(pattern: &Node, text: &'a str, names: &mut Vec<&'a str>) {
    for child in named_children(pattern) {
        match child.kind() {
            "record_pattern_body" | "record_pattern" => record_pattern_variables(&child, text, names),
            "record_pattern_component" => {
                let parts = named_children(&child);
                match parts.iter().find(|p| p.kind() == "record_pattern") {
                    Some(nested) => record_pattern_variables(nested, text, names),
                    None => names.extend(parts.iter().filter(|p| p.kind() == "identifier").map(|p| source(p, text))),
                }
            }
            _ => {}
        }
    }
}

/// The expression, `return` and `if` statements directly after the statement, up to any other statement (also an
/// empty one, `;`).
fn following_statements<'t>(statement: &Node<'t>) -> Vec<Node<'t>> {
    let mut statements = Vec::new();
    let mut next = statement.next_sibling();
    while let Some(sibling) = next {
        match sibling.kind() {
            "expression_statement" | "return_statement" | "if_statement" => statements.push(sibling),
            _ if is_comment(&sibling) => {}
            _ => break,
        }
        next = sibling.next_sibling();
    }
    statements
}

fn report_assignments(branches: &[Node], names: &[&str], text: &str, diagnostics: &mut Vec<Diagnostic>) {
    for branch in branches {
        let mut visit = |n: &Node| {
            let target = match n.kind() {
                "assignment_expression" => n.child_by_field_name("left"),
                "update_expression" => n.named_children(&mut n.walk()).find(|c| !is_comment(c)),
                _ => None,
            };
            if let Some(target) = target
                && target.kind() == "identifier"
                && names.contains(&source(&target, text))
            {
                diagnostics.push(Diagnostic::new(
                    Rule::PatternVariableAssignment,
                    target.start_byte(),
                    format!("Assignment of pattern variable '{}' is not allowed", source(&target, text)),
                ));
            }
        };
        visit(branch);
        for_each_descendant(branch, &[], &mut visit);
    }
}
