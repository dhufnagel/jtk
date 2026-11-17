//! C005: a method, constructor, initializer or lambda with too many statements (Checkstyle
//! ExecutableStatementCount).
//!
//! Follows Checkstyle's count: for each statement list (a block or the statements of a `case` group) in the method
//! but outside nested methods and lambdas, half the number of nodes in Checkstyle's syntax tree of the list, rounded
//! down. An expression statement or a local variable declaration contributes its expression or each variable and a
//! separator, other statements one node, and a block its closing brace; so `{ if (a) {} if (b) {} }` counts 1.

use tree_sitter::Node;

use super::{CaseGroups, Event, Visitor, is_method_like, kinds};
use crate::lint::syntax::is_comment;
use crate::lint::{Diagnostic, Rule};

pub fn visitor<'t, 'a>(_text: &'a str, max: u32, diagnostics: &'a mut Vec<Diagnostic>) -> Visitor<'t, 'a> {
    // The counts of the enclosing methods and lambdas; the first collects the code outside them.
    let mut counts: Vec<u32> = vec![0];
    let mut case_groups = CaseGroups::default();
    // The nodes of the statement list of the `case` group being visited.
    let mut group_nodes = 0;
    let kinds = kinds![
        "method_declaration",
        "constructor_declaration",
        "compact_constructor_declaration",
        "static_initializer",
        "block",
        "lambda_expression",
        "constructor_body",
        "switch_block",
        "switch_block_statement_group",
    ];
    let visit = move |event: Event, node: &Node<'t>, ancestors: &[Node<'t>]| {
        let parent = ancestors.last();
        let is_container = is_method_like(node, parent) || node.kind() == "lambda_expression";
        match event {
            Event::Enter => {
                if is_container {
                    counts.push(0);
                }
                let nodes = match node.kind() {
                    "block" | "constructor_body" => Some(statement_nodes(node) + 1),
                    "switch_block" => {
                        case_groups.add(node);
                        None
                    }
                    "switch_block_statement_group" => {
                        if case_groups.get(node).starts {
                            group_nodes = 0;
                        }
                        group_nodes += statement_nodes(node);
                        case_groups.get(node).ends.then_some(group_nodes)
                    }
                    _ => None,
                };
                if let (Some(nodes), Some(count)) = (nodes, counts.last_mut()) {
                    *count += nodes / 2;
                }
            }
            Event::Leave if is_container && counts.len() > 1 => {
                let count = counts.pop().unwrap_or(0);
                if count > max {
                    let position = match node.kind() {
                        "lambda_expression" => super::token(node, "->").map_or(node.start_byte(), |t| t.start_byte()),
                        _ => node.start_byte(),
                    };
                    let message = format!("Executable statement count is {count} (max allowed is {max}).");
                    diagnostics.push(Diagnostic::new(Rule::ExecutableStatementCount, position, message));
                }
            }
            Event::Leave => {}
        }
    };
    (kinds, Box::new(visit))
}

/// The number of nodes that the statements directly in the block or group have in Checkstyle's statement list.
fn statement_nodes(list: &Node) -> u32 {
    let mut nodes = 0;
    for child in list.children(&mut list.walk()) {
        nodes += match child.kind() {
            "{" | "}" | "switch_label" | ":" | "->" => 0,
            _ if is_comment(&child) => 0,
            "expression_statement" => 2,
            "local_variable_declaration" => {
                2 * child.children_by_field_name("declarator", &mut child.walk()).count() as u32
            }
            _ => 1,
        };
    }
    nodes
}
