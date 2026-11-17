//! S006: parentheses that change nothing (Checkstyle UnnecessaryParentheses, PMD UselessParentheses, Error Prone
//! UnnecessaryParentheses): around a name or literal, a whole value (returned, assigned, passed as argument, lambda
//! body, `case` label, guard or assertion), a single lambda parameter, other parentheses, or an operation that binds
//! tighter than the one around it (`(a && b) || c`). Like Checkstyle, associativity is not considered: `(a + b) + c`
//! is not reported.

use tree_sitter::Node;

use crate::lint::syntax::{is_comment, named_children, operator, source};
use crate::lint::{Diagnostic, Edit, Rule};

const LITERALS: &[&str] = &[
    "decimal_integer_literal",
    "hex_integer_literal",
    "octal_integer_literal",
    "binary_integer_literal",
    "decimal_floating_point_literal",
    "hex_floating_point_literal",
    "string_literal",
    "character_literal",
    "null_literal",
    "true",
    "false",
];

/// Statements whose parentheses belong to their syntax.
const PARENTHESIZED_SYNTAX: &[&str] =
    &["if_statement", "while_statement", "do_statement", "switch_expression", "synchronized_statement"];

pub fn check(node: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    match node.kind() {
        "parenthesized_expression" => check_parentheses(node, text, diagnostics),
        "lambda_expression" => check_lambda_parameter(node, text, diagnostics),
        _ => {}
    }
}

fn check_parentheses(parentheses: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(parent) = parentheses.parent() else { return };
    let inner: Vec<Node> = named_children(parentheses).into_iter().filter(|c| !is_comment(c)).collect();
    let ([inner], false) = (inner.as_slice(), named_children(parentheses).iter().any(is_comment)) else { return };
    if PARENTHESIZED_SYNTAX.contains(&parent.kind()) {
        return;
    }
    let message = if inner.kind() == "identifier" {
        format!("Unnecessary parentheses around identifier '{}'", source(inner, text))
    } else if LITERALS.contains(&inner.kind()) {
        format!("Unnecessary parentheses around literal '{}'", source(inner, text))
    } else if parent.kind() == "parenthesized_expression" {
        "Unnecessary parentheses around expression".to_string()
    } else if is_whole_value(parentheses, &parent) {
        "Unnecessary parentheses around a whole value".to_string()
    } else if binds_tighter(parentheses, inner, &parent, text) {
        "Unnecessary parentheses around expression".to_string()
    } else {
        return;
    };
    let fix = Edit::new(parentheses.byte_range(), source(inner, text));
    diagnostics
        .push(Diagnostic::new(Rule::UnnecessaryParentheses, parentheses.start_byte(), message).with_fix(vec![fix]));
}

/// The whole value of a `return`, `yield`, variable initializer, assignment, argument, lambda body, `case` label,
/// guard or assertion.
fn is_whole_value(parentheses: &Node, parent: &Node) -> bool {
    match parent.kind() {
        "return_statement" | "yield_statement" | "argument_list" | "guard" | "switch_label" | "assert_statement" => {
            true
        }
        "lambda_expression" => parent.child_by_field_name("body") == Some(*parentheses),
        "for_statement" => parent.child_by_field_name("condition") == Some(*parentheses),
        "variable_declarator" => parent.child_by_field_name("value") == Some(*parentheses),
        "assignment_expression" => parent.child_by_field_name("right") == Some(*parentheses),
        _ => false,
    }
}

/// Operations whose parentheses Checkstyle reports by default when they bind tighter than the operation around
/// them; arithmetic and bitwise operations are not reported.
const CONDITIONAL_OPERATORS: &[&str] = &["&&", "||", "==", "!=", "<", ">", "<=", ">="];

/// Expressions that bind tighter than any operator.
const PRIMARY: &[&str] = &["method_invocation", "field_access", "array_access", "cast_expression", "this"];

/// Whether the parenthesized expression binds tighter than the operation around it: a comparison or logical
/// operation inside `&&` or `||`, a unary operation inside a comparison or logical operation, or a call or member
/// access inside any operation (a cast too, unless a member of it is selected). The condition of `?:` may keep its
/// parentheses for clarity.
fn binds_tighter(parentheses: &Node, inner: &Node, parent: &Node, text: &str) -> bool {
    let is_primary = PRIMARY.contains(&inner.kind());
    let outer = match parent.kind() {
        "binary_expression" => operator(parent, text),
        "ternary_expression" => return parent.child_by_field_name("condition") != Some(*parentheses) && is_primary,
        "unary_expression" | "cast_expression" => return is_primary,
        "method_invocation" | "field_access" => {
            let is_object = parent.child_by_field_name("object") == Some(*parentheses);
            return is_object && is_primary && inner.kind() != "cast_expression";
        }
        _ => return false,
    };
    let is_conditional_context = CONDITIONAL_OPERATORS.contains(&outer);
    match inner.kind() {
        "binary_expression" => {
            let op = operator(inner, text);
            // `p == (q <= r)` may need its parentheses: comparing booleans is not reported.
            matches!(outer, "&&" | "||")
                && CONDITIONAL_OPERATORS.contains(&op)
                && (precedence(op) > precedence(outer) || op == outer)
        }
        "instanceof_expression" => is_conditional_context && precedence("instanceof") > precedence(outer),
        "unary_expression" | "update_expression" => is_conditional_context,
        kind => PRIMARY.contains(&kind),
    }
}

fn precedence(operator: &str) -> u8 {
    match operator {
        "||" => 4,
        "&&" => 5,
        "|" => 6,
        "^" => 7,
        "&" => 8,
        "==" | "!=" => 9,
        "<" | ">" | "<=" | ">=" | "instanceof" => 10,
        "<<" | ">>" | ">>>" => 11,
        "+" | "-" => 12,
        _ => 13,
    }
}

/// `(x) -> ...`: a single parameter without type needs no parentheses.
fn check_lambda_parameter(lambda: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(parameters) = lambda.child_by_field_name("parameters") else { return };
    let names = named_children(&parameters);
    if parameters.kind() != "inferred_parameters" || names.len() != 1 {
        return;
    }
    let fix = Edit::new(parameters.byte_range(), source(&names[0], text));
    let message = "Unnecessary parentheses around lambda parameter";
    diagnostics
        .push(Diagnostic::new(Rule::UnnecessaryParentheses, parameters.start_byte(), message).with_fix(vec![fix]));
}
