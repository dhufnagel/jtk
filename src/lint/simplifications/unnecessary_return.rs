//! S010: a `return;` that is the last thing a method, constructor, initializer or lambda does (PMD
//! UnnecessaryReturn): every statement enclosing it is the last of its parent (or the branch of an `if` or the body of
//! a `try`), outside loops and switch expressions.
//!
//! The fix removes the statement (its whole line if it is alone on it) when it is in a block or a `case` group;
//! elsewhere (`if (a) return;`) removing it would leave a statement without body.

use tree_sitter::Node;

use super::{contains_comment, line_removal_range};
use crate::lint::syntax::is_comment;
use crate::lint::{Diagnostic, Rule};

pub fn check(statement: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if statement.named_child_count() > 0 && statement.named_children(&mut statement.walk()).any(|c| !is_comment(&c)) {
        return;
    }
    let mut node = *statement;
    let Some(container) = statement.parent() else { return };
    let mut parent = container;
    loop {
        if matches!(
            parent.kind(),
            "method_declaration"
                | "constructor_declaration"
                | "compact_constructor_declaration"
                | "lambda_expression"
                | "static_initializer"
                | "class_body"
                | "enum_body_declarations"
        ) {
            break;
        }
        if is_statement(&node) && !is_last_statement(&node, &parent) {
            return;
        }
        node = parent;
        let Some(next) = node.parent() else { return };
        parent = next;
    }
    let diagnostic = Diagnostic::new(Rule::UnnecessaryReturn, statement.start_byte(), "Unnecessary return statement");
    let removable = matches!(container.kind(), "block" | "constructor_body" | "switch_block_statement_group");
    diagnostics.push(if removable && !contains_comment(statement) {
        diagnostic.with_fix(vec![line_removal_range(statement, text)])
    } else {
        diagnostic
    });
}

fn is_statement(node: &Node) -> bool {
    let kind = node.kind();
    kind.ends_with("_statement") || matches!(kind, "block" | "constructor_body" | "switch_expression")
}

/// Whether the statement is the last thing its parent does before control leaves it.
fn is_last_statement(statement: &Node, parent: &Node) -> bool {
    if !is_last_child(statement) {
        return matches!(parent.kind(), "if_statement" | "try_statement" | "try_with_resources_statement");
    }
    match parent.kind() {
        "while_statement" | "for_statement" | "enhanced_for_statement" | "do_statement" => false,
        "switch_block_statement_group" => is_last_child(parent) && !in_switch_expression(parent),
        "switch_rule" => !in_switch_expression(parent),
        _ => true,
    }
}

/// Whether the `case` of a switch belongs to a switch expression rather than a switch statement.
fn in_switch_expression(case: &Node) -> bool {
    let switch = case.parent().and_then(|block| block.parent());
    let position = switch.and_then(|s| s.parent());
    position.is_some_and(|p| {
        !matches!(
            p.kind(),
            "block"
                | "constructor_body"
                | "switch_block_statement_group"
                | "labeled_statement"
                | "if_statement"
                | "while_statement"
                | "for_statement"
                | "enhanced_for_statement"
                | "do_statement"
        )
    })
}

fn is_last_child(node: &Node) -> bool {
    std::iter::successors(node.next_named_sibling(), |n| n.next_named_sibling()).all(|n| is_comment(&n))
}
