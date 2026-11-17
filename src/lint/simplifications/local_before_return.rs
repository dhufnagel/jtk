//! S005: a local variable that is only declared to be returned by the next statement (PMD
//! UnnecessaryLocalBeforeReturn with its defaults): `T x = e; return x;` can be `return e;`. Reported on the variable,
//! like PMD; not if the variable is annotated or used elsewhere.
//!
//! The fix is left out where returning the expression directly could convert it differently: unless the variable's
//! type is written exactly like the method's return type (not in lambdas, not with `var`). It is also left out for
//! array initializers (`int[] a = {1}`), several variables in one declaration, and comments that would be lost.

use tree_sitter::Node;

use super::comment_in;
use crate::lint::syntax::{for_each_descendant, is_comment, modifiers, named_children, source, strip_parentheses};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(statement: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(value) = statement.named_child(0).map(|v| strip_parentheses(&v)).filter(|v| v.kind() == "identifier")
    else {
        return;
    };
    let name = source(&value, text);
    let Some(declaration) = std::iter::successors(statement.prev_named_sibling(), |n| n.prev_named_sibling())
        .find(|n| !is_comment(n))
        .filter(|n| n.kind() == "local_variable_declaration")
    else {
        return;
    };
    let declarators: Vec<Node> = declaration.children_by_field_name("declarator", &mut declaration.walk()).collect();
    let Some(position) =
        declarators.iter().position(|d| d.child_by_field_name("name").is_some_and(|n| source(&n, text) == name))
    else {
        return;
    };
    let declarator = declarators[position];
    let Some(initializer) = declarator.child_by_field_name("value") else { return };
    let annotated = modifiers(&declaration)
        .is_some_and(|m| named_children(&m).iter().any(|a| matches!(a.kind(), "annotation" | "marker_annotation")));
    if annotated
        || declarators[position + 1..].iter().any(|d| uses(d, name, text))
        || used_in_later_cases(statement, name, text)
    {
        return;
    }
    let message = format!("Consider simply returning the value vs storing it in local variable '{name}'");
    let Some(name_node) = declarator.child_by_field_name("name") else { return };
    let diagnostic = Diagnostic::new(Rule::LocalBeforeReturn, name_node.start_byte(), message);
    let fixable = declarators.len() == 1
        && initializer.kind() != "array_initializer"
        && declarator.child_by_field_name("dimensions").is_none()
        && !comment_in(&declaration, declaration.byte_range())
        && !comment_in(statement, statement.byte_range())
        && text[declaration.end_byte()..statement.start_byte()].trim().is_empty()
        && declaration.child_by_field_name("type").is_some_and(|t| {
            source(&t, text) != "var" && return_type(statement).is_some_and(|r| same_type(&r, &t, text))
        });
    diagnostics.push(if fixable {
        let fix = Edit::new(
            declaration.start_byte()..statement.end_byte(),
            format!("return {};", source(&initializer, text)),
        );
        diagnostic.with_fix(vec![fix])
    } else {
        diagnostic
    });
}

fn uses(node: &Node, name: &str, text: &str) -> bool {
    let mut found = false;
    for_each_descendant(node, &[], |n| found |= n.kind() == "identifier" && source(n, text) == name);
    found
}

/// Whether the variable is used in the following groups of a `switch`, where it is still in scope.
fn used_in_later_cases(statement: &Node, name: &str, text: &str) -> bool {
    let Some(group) = statement.parent().filter(|p| p.kind() == "switch_block_statement_group") else { return false };
    std::iter::successors(group.next_named_sibling(), |n| n.next_named_sibling()).any(|g| uses(&g, name, text))
}

/// The declared return type of the method that the statement returns from, unless it is in a lambda.
fn return_type<'t>(statement: &Node<'t>) -> Option<Node<'t>> {
    let method = std::iter::successors(statement.parent(), |n| n.parent())
        .find(|n| matches!(n.kind(), "method_declaration" | "lambda_expression" | "class_body"))?;
    (method.kind() == "method_declaration" && method.child_by_field_name("dimensions").is_none())
        .then(|| method.child_by_field_name("type"))
        .flatten()
}

fn same_type(a: &Node, b: &Node, text: &str) -> bool {
    let normalized = |n: &Node| source(n, text).split_whitespace().collect::<String>();
    normalized(a) == normalized(b)
}
