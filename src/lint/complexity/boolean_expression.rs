//! C003: an expression with too many boolean operators (Checkstyle BooleanExpressionComplexity).
//!
//! As in Checkstyle, counts `&&`, `||`, `&`, `|` and `^` (`&`, `|` and `^` also on numbers, and not when they are
//! the outermost operator of a method argument) in each expression of a method or constructor other than `equals`.
//! An expression is what Checkstyle's syntax tree wraps in an EXPR node: an expression statement, a condition, a
//! return value, a variable initializer, a method argument, an index, a lambda's expression body, and so on;
//! operators in a nested expression (an argument of a call within the expression) count for that one only. Code
//! outside methods (field initializers, initializer blocks) is not checked.
//!
//! The violation is reported where Checkstyle reports it, on the expression's parent in its syntax tree: for an
//! expression statement that is the opening brace of the enclosing block, for a method argument the first argument.

use tree_sitter::Node;

use super::{Event, Visitor, kinds, token};
use crate::lint::syntax::{has_modifier, is_comment, operator};
use crate::lint::{Diagnostic, Rule};

pub fn visitor<'t, 'a>(text: &'a str, max: u32, diagnostics: &'a mut Vec<Diagnostic>) -> Visitor<'t, 'a> {
    // The expressions with counted operators, in the order of their first operator: (expression id, position, count).
    let mut expressions: Vec<(usize, usize, u32)> = Vec::new();
    let visit = move |event: Event, node: &Node<'t>, ancestors: &[Node<'t>]| {
        if node.kind() == "program" {
            if event == Event::Leave {
                for &(_, position, count) in expressions.iter().filter(|e| e.2 > max) {
                    let message = format!("Boolean expression complexity is {count} (max allowed is {max}).");
                    diagnostics.push(Diagnostic::new(Rule::BooleanExpressionComplexity, position, message));
                }
                expressions.clear();
            }
            return;
        }
        if event != Event::Enter || !is_counted_operator(node, ancestors, text) || !is_checked(ancestors, text) {
            return;
        }
        let Some((id, position)) = enclosing_expression(node, ancestors) else { return };
        match expressions.iter_mut().find(|e| e.0 == id) {
            Some(expression) => expression.2 += 1,
            None => expressions.push((id, position, 1)),
        }
    };
    (kinds!["program", "binary_expression"], Box::new(visit))
}

/// The innermost expression that holds the operator (or is it), as its id and where it is reported.
fn enclosing_expression(operator: &Node, ancestors: &[Node]) -> Option<(usize, usize)> {
    if let Some(position) = expression_position(operator, ancestors) {
        return Some((operator.id(), position));
    }
    (0..ancestors.len())
        .rev()
        .find_map(|i| expression_position(&ancestors[i], &ancestors[..i]).map(|p| (ancestors[i].id(), p)))
}

/// Whether code with these ancestors is checked: it must be in a method or constructor, and the innermost one must
/// not be an `equals` method (whose body typically chains comparisons of all fields).
fn is_checked(ancestors: &[Node], text: &str) -> bool {
    let method = ancestors.iter().rev().find(|a| {
        matches!(a.kind(), "method_declaration" | "constructor_declaration" | "compact_constructor_declaration")
    });
    method.is_some_and(|m| !is_equals_method(m, text))
}

/// Whether the declaration is an instance method `equals` with one parameter.
fn is_equals_method(method: &Node, text: &str) -> bool {
    let is_named_equals = method.child_by_field_name("name").is_some_and(|n| &text[n.byte_range()] == "equals");
    let parameters = method.child_by_field_name("parameters").map_or(0, |list| {
        list.named_children(&mut list.walk())
            .filter(|p| matches!(p.kind(), "formal_parameter" | "spread_parameter"))
            .count()
    });
    method.kind() == "method_declaration" && is_named_equals && parameters == 1 && !has_modifier(method, text, "static")
}

fn is_counted_operator(node: &Node, ancestors: &[Node], text: &str) -> bool {
    if node.kind() != "binary_expression" {
        return false;
    }
    match operator(node, text) {
        "&&" | "||" => true,
        "&" | "|" | "^" => {
            // Checkstyle ignores the outermost operator of a method argument.
            let parent = ancestors.iter().rev().find(|a| a.kind() != "parenthesized_expression");
            parent.is_none_or(|p| p.kind() != "argument_list")
        }
        _ => false,
    }
}

/// If the node is an expression that Checkstyle wraps in an EXPR node, where it reports it: on the EXPR's parent.
fn expression_position(node: &Node, ancestors: &[Node]) -> Option<usize> {
    let parent = ancestors.last()?;
    let grandparent = ancestors.len().checked_sub(2).map(|i| ancestors[i]);
    let field = field_name(parent, node);
    match parent.kind() {
        "expression_statement" => {
            let grandparent = grandparent?;
            Some(match grandparent.kind() {
                "switch_block_statement_group" => first_statement_position(&grandparent),
                "if_statement" if grandparent.child_by_field_name("alternative") == Some(*parent) => {
                    token(&grandparent, "else").map_or(grandparent.start_byte(), |t| t.start_byte())
                }
                "labeled_statement" => token(&grandparent, ":").map_or(grandparent.start_byte(), |t| t.start_byte()),
                _ => grandparent.start_byte(),
            })
        }
        "parenthesized_expression" => {
            let grandparent = grandparent?;
            matches!(
                grandparent.kind(),
                "if_statement" | "while_statement" | "do_statement" | "switch_expression" | "synchronized_statement"
            )
            .then(|| grandparent.start_byte())
        }
        "return_statement" | "throw_statement" | "yield_statement" | "assert_statement" | "switch_label" | "guard"
        | "dimensions_expr" => {
            let is_pattern = matches!(node.kind(), "pattern" | "type_pattern" | "record_pattern" | "guard");
            (!is_pattern).then(|| parent.start_byte())
        }
        "variable_declarator" | "resource" if field == Some("value") => {
            Some(token(parent, "=").map_or(parent.start_byte(), |t| t.start_byte()))
        }
        "argument_list" if node.kind() != "lambda_expression" => {
            let first = parent.named_children(&mut parent.walk()).find(|c| !is_comment(c))?;
            Some(root_position(&first))
        }
        "array_access" if field == Some("index") => {
            Some(token(parent, "[").map_or(node.start_byte(), |t| t.start_byte()))
        }
        "array_initializer" if node.kind() != "array_initializer" => Some(parent.start_byte()),
        "for_statement" => match field? {
            "condition" => Some(root_position(node)),
            "init" | "update" if node.kind() != "local_variable_declaration" => {
                let first = parent.children_by_field_name(field?, &mut parent.walk()).next()?;
                Some(root_position(&first))
            }
            _ => None,
        },
        "enhanced_for_statement" if field == Some("value") => {
            parent.named_children(&mut parent.walk()).find(|c| !is_comment(c)).map(|c| c.start_byte())
        }
        "lambda_expression" if field == Some("body") && node.kind() != "block" => {
            Some(token(parent, "->").map_or(parent.start_byte(), |t| t.start_byte()))
        }
        "element_value_pair" if field == Some("value") => Some(parent.start_byte()),
        "annotation_argument_list" if node.kind() != "element_value_pair" => grandparent.map(|g| g.start_byte()),
        _ => None,
    }
}

/// The name of the field of `parent` that holds `child`.
fn field_name<'t>(parent: &Node<'t>, child: &Node<'t>) -> Option<&'t str> {
    let mut cursor = parent.walk();
    if !cursor.goto_first_child() {
        return None;
    }
    loop {
        if cursor.node() == *child {
            return cursor.field_name();
        }
        if !cursor.goto_next_sibling() {
            return None;
        }
    }
}

/// The position of Checkstyle's first statement of a `case` group, which is where it reports the group's statement
/// list.
fn first_statement_position(group: &Node) -> usize {
    let first = group.named_children(&mut group.walk()).find(|c| c.kind() != "switch_label" && !is_comment(c));
    match first {
        Some(statement) if statement.kind() == "expression_statement" => {
            statement.named_child(0).map_or(statement.start_byte(), |e| root_position(&e))
        }
        Some(statement) => statement.start_byte(),
        None => group.start_byte(),
    }
}

/// The position of the token at the root of Checkstyle's syntax tree of an expression, such as the operator of a
/// binary expression or the opening parenthesis of a method call.
fn root_position(expression: &Node) -> usize {
    let at = |kind: &str| token(expression, kind).map_or(expression.start_byte(), |t| t.start_byte());
    match expression.kind() {
        "binary_expression" | "assignment_expression" => {
            expression.child_by_field_name("operator").map_or(expression.start_byte(), |o| o.start_byte())
        }
        "ternary_expression" => at("?"),
        "method_invocation" => {
            expression.child_by_field_name("arguments").map_or(expression.start_byte(), |a| a.start_byte())
        }
        "field_access" | "class_literal" => at("."),
        "array_access" => at("["),
        "instanceof_expression" => at("instanceof"),
        "lambda_expression" => at("->"),
        "method_reference" => at("::"),
        "object_creation_expression" => at("new"),
        "update_expression" => {
            let first = expression.child(0);
            match first.filter(|f| f.is_named()) {
                Some(_) => expression.child(1).map_or(expression.start_byte(), |o| o.start_byte()),
                None => expression.start_byte(),
            }
        }
        _ => expression.start_byte(),
    }
}
