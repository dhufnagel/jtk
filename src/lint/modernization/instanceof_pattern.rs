//! M001: `instanceof` followed by a cast to a new variable (Eclipse "Use pattern matching for instanceof", IntelliJ
//! "Pattern variable can be used"), Java 16:
//!
//! `if (o instanceof String) { String s = (String) o; ... }` becomes `if (o instanceof String s) { ... }`.
//!
//! The `instanceof` is the condition or part of an `&&` condition, the declaration is the first statement of the
//! `if` block, and the tested expression is a name or field, so that casting it again gives the same object. The
//! rest of the condition must not use the variable's name, which would then refer to the pattern variable.

use tree_sitter::Node;

use crate::lint::syntax::{
    for_each_descendant, is_comment, modifiers, named_children, operator, source, strip_parentheses,
};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(statement: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (Some(condition), Some(consequence)) =
        (statement.child_by_field_name("condition"), statement.child_by_field_name("consequence"))
    else {
        return;
    };
    let Some(declaration) = named_children(&consequence).into_iter().find(|c| !is_comment(c)) else { return };
    if consequence.kind() != "block"
        || declaration.kind() != "local_variable_declaration"
        || modifiers(&declaration).is_some()
    {
        return;
    }
    let Some((name, cast)) = cast_declaration(&declaration, text) else { return };
    for test in conjuncts(&condition, text).into_iter().filter(|t| t.kind() == "instanceof_expression") {
        let (Some(tested), Some(test_type)) = (test.child_by_field_name("left"), test.child_by_field_name("right"))
        else {
            continue;
        };
        let matches = test.child_by_field_name("name").is_none()
            && matches!(strip_parentheses(&tested).kind(), "identifier" | "field_access")
            && source(&test_type, text) == cast.0
            && source(&strip_parentheses(&tested), text) == cast.1
            && !names_after(&condition, test.end_byte(), name, text);
        if matches {
            let message = format!("instanceof and cast can be a pattern variable: instanceof {} {name}", cast.0);
            let fix = vec![
                Edit::new(test_type.end_byte()..test_type.end_byte(), format!(" {name}")),
                Edit::new(statement_lines(&declaration, text), ""),
            ];
            diagnostics.push(Diagnostic::new(Rule::InstanceofPattern, test.start_byte(), message).with_fix(fix));
            return;
        }
    }
}

/// Whether the condition uses `name` after `position`: the pattern variable would take its place.
fn names_after(condition: &Node, position: usize, name: &str, text: &str) -> bool {
    let mut found = false;
    for_each_descendant(condition, &[], |n| {
        found |= n.kind() == "identifier" && n.start_byte() >= position && source(n, text) == name;
    });
    found
}

/// `T name = (T) x;`: the name, and the cast's type and operand.
fn cast_declaration<'a>(declaration: &Node, text: &'a str) -> Option<(&'a str, (&'a str, &'a str))> {
    let declarators: Vec<Node> =
        named_children(declaration).into_iter().filter(|d| d.kind() == "variable_declarator").collect();
    let [declarator] = declarators.as_slice() else { return None };
    let declared_type = source(&declaration.child_by_field_name("type")?, text);
    let cast = strip_parentheses(&declarator.child_by_field_name("value")?);
    if cast.kind() != "cast_expression" {
        return None;
    }
    let cast_type = source(&cast.child_by_field_name("type")?, text);
    let operand = source(&strip_parentheses(&cast.child_by_field_name("value")?), text);
    let name = source(&declarator.child_by_field_name("name")?, text);
    (declared_type == cast_type || declared_type == "var").then_some((name, (cast_type, operand)))
}

/// The operands of a condition's top-level `&&` chain (the condition itself if it is not one).
fn conjuncts<'t>(condition: &Node<'t>, text: &str) -> Vec<Node<'t>> {
    let condition = strip_parentheses(condition);
    if condition.kind() == "binary_expression" && operator(&condition, text) == "&&" {
        let sides = [condition.child_by_field_name("left"), condition.child_by_field_name("right")];
        return sides.iter().flatten().flat_map(|side| conjuncts(side, text)).collect();
    }
    vec![condition]
}

/// The statement's range, extended to its whole lines when nothing else is on them.
fn statement_lines(statement: &Node, text: &str) -> std::ops::Range<usize> {
    let line_start = text[..statement.start_byte()].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[statement.end_byte()..].find('\n').map_or(text.len(), |i| statement.end_byte() + i + 1);
    let alone_on_lines = text[line_start..statement.start_byte()].trim().is_empty()
        && text[statement.end_byte()..line_end].trim().is_empty();
    if alone_on_lines { line_start..line_end } else { statement.byte_range() }
}
