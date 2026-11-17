//! B019: double-checked locking on a field that is not `volatile` (PMD DoubleCheckedLocking): other threads can see
//! the reference before the object is fully constructed.
//!
//! `if (x == null) { synchronized (...) { if (x == null) { x = ...; } } }`
//!
//! The outer check may be negated (`if (!(x != null))`). PMD reports the method; jtk reports the outer `if`, which
//! points at the pattern (like Error Prone's DoubleCheckedLocking). PMD also requires a non-primitive return type and a
//! single `return` of the field, which jtk does not: the unsafe publication does not depend on them.

use tree_sitter::Node;

use crate::lint::syntax::{
    NESTED_CODE, descendants, has_modifier, named_children, operator, source, strip_parentheses,
};
use crate::lint::{Diagnostic, Rule};

pub fn check(outer: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(field) = null_tested(outer, text) else { return };
    let Some(consequence) = outer.child_by_field_name("consequence") else { return };
    let inner_checks = descendants(&consequence, "synchronized_statement", NESTED_CODE)
        .iter()
        .flat_map(|s| descendants(s, "if_statement", NESTED_CODE))
        .filter(|inner| null_tested(inner, text) == Some(field))
        .any(|inner| assigns(&inner, field, text));
    if inner_checks && is_non_volatile_field(outer, field, text) {
        let message = format!("Double-checked locking on '{field}', which is not volatile");
        diagnostics.push(Diagnostic::new(Rule::DoubleCheckedLocking, outer.start_byte(), message));
    }
}

/// The field name `x` in an `if (x == null)` or `if (this.x == null)` condition, or its negation `!(x != null)`.
fn null_tested<'a>(statement: &Node, text: &'a str) -> Option<&'a str> {
    let mut condition = strip_parentheses(&statement.child_by_field_name("condition")?);
    let mut negated = false;
    while condition.kind() == "unary_expression" && operator(&condition, text) == "!" {
        condition = strip_parentheses(&condition.child_by_field_name("operand")?);
        negated = !negated;
    }
    if operator(&condition, text) != if negated { "!=" } else { "==" } {
        return None;
    }
    let left = strip_parentheses(&condition.child_by_field_name("left")?);
    let right = strip_parentheses(&condition.child_by_field_name("right")?);
    let tested = if right.kind() == "null_literal" {
        left
    } else if left.kind() == "null_literal" {
        right
    } else {
        return None;
    };
    field_name(&tested, text)
}

fn field_name<'a>(expression: &Node, text: &'a str) -> Option<&'a str> {
    match expression.kind() {
        "identifier" => Some(source(expression, text)),
        "field_access" if expression.child_by_field_name("object").is_some_and(|o| o.kind() == "this") => {
            Some(source(&expression.child_by_field_name("field")?, text))
        }
        _ => None,
    }
}

fn assigns(statement: &Node, field: &str, text: &str) -> bool {
    descendants(statement, "assignment_expression", NESTED_CODE)
        .iter()
        .any(|a| a.child_by_field_name("left").is_some_and(|l| field_name(&l, text) == Some(field)))
}

/// Whether `field` is declared in the class enclosing `node`, without `volatile`.
fn is_non_volatile_field(node: &Node, field: &str, text: &str) -> bool {
    let Some(class_body) = std::iter::successors(node.parent(), |n| n.parent()).find(|n| n.kind() == "class_body")
    else {
        return false;
    };
    let declaration = named_children(&class_body).into_iter().filter(|m| m.kind() == "field_declaration").find(|m| {
        named_children(m)
            .iter()
            .filter(|d| d.kind() == "variable_declarator")
            .any(|d| d.child_by_field_name("name").is_some_and(|n| source(&n, text) == field))
    });
    declaration.is_some_and(|d| !has_modifier(&d, text, "volatile"))
}
