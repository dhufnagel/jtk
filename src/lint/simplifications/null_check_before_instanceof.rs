//! S020: `x != null` combined with `&&` with `x instanceof T`, which is false for `null` anyway (Checkstyle
//! UnnecessaryNullCheckWithInstanceOf, PMD SimplifyConditional). Following Checkstyle, the null check may be anywhere
//! in the `&&` chain, but not after `x` was dereferenced (`x != null && x.isValid() && x instanceof T`), and not in an
//! `||` alternative to the `instanceof`. Like Checkstyle, only conditions whose outermost `&&`/`||` operation is an
//! `&&` are checked (not `x != null && x instanceof T || y`), and a null check before the `instanceof` is reported
//! even if the `instanceof` is in an `||` inside the chain (`x != null && (x instanceof T || y)`). A null check after
//! the `instanceof` is reported only if the `instanceof` is itself in the chain.
//!
//! The fix removes the null check if it is next to the `instanceof` in the chain, so that nothing else is evaluated
//! with a `null` `x` that was not before.

use tree_sitter::Node;

use super::comment_in;
use crate::lint::syntax::{for_each_descendant, operator, source, strip_parentheses};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(instanceof: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(name) = tested_variable(instanceof, text) else { return };
    let top = outermost_condition(instanceof, text);
    if !is_operation(&top, text, "&&") {
        return;
    }
    // Several `instanceof`s of the variable in one condition share its null check: report it once, for the first.
    let Some((first, null_check)) = redundant_null_check(&top, name, text) else { return };
    if first != *instanceof {
        return;
    }
    let mut diagnostic = Diagnostic::new(
        Rule::NullCheckBeforeInstanceof,
        null_check.start_byte(),
        "Unnecessary null check before instanceof",
    );
    if let Some(edit) = removal(&top, &null_check, instanceof, text) {
        diagnostic = diagnostic.with_fix(vec![edit]);
    }
    diagnostics.push(diagnostic);
}

/// The first `instanceof` of the variable in the `&&` condition `top` that makes a null check of it in the `&&`
/// chain redundant, with that null check.
///
/// The `instanceof` must come before any dereference of the variable in the condition (which the null check guards
/// against). A null check before the `instanceof` is reported also when the `instanceof` is in an `||` alternative,
/// as in Checkstyle; a null check after it only when the `instanceof` is itself in the `&&` chain, so that it is
/// true whenever the null check is evaluated.
fn redundant_null_check<'t>(top: &Node<'t>, name: &str, text: &str) -> Option<(Node<'t>, Node<'t>)> {
    let mut first_dereference = usize::MAX;
    let mut instanceofs = Vec::new();
    for_each_descendant(top, &[], |node| match node.kind() {
        "field_access" | "method_invocation"
            if node
                .child_by_field_name("object")
                .is_some_and(|o| o.kind() == "identifier" && source(&o, text) == name) =>
        {
            first_dereference = first_dereference.min(node.start_byte());
        }
        "instanceof_expression" if tested_variable(node, text) == Some(name) => instanceofs.push(*node),
        _ => {}
    });
    let null_checks: Vec<Node> = flatten(top, text)
        .into_iter()
        .map(|(leaf, _)| strip_parentheses(&leaf))
        .filter(|l| is_null_check(l, name, text))
        .collect();
    instanceofs
        .into_iter()
        .filter(|i| i.start_byte() < first_dereference && outermost_condition(i, text) == *top)
        .find_map(|instanceof| {
            let in_chain = is_in_and_chain(&instanceof, top, text);
            let null_check = null_checks.iter().find(|n| n.start_byte() < instanceof.start_byte() || in_chain)?;
            Some((instanceof, *null_check))
        })
}

/// Whether the node is an operand of the `&&` chain `top`: only parentheses and `&&` operations lie between them.
fn is_in_and_chain(node: &Node, top: &Node, text: &str) -> bool {
    let mut current = *node;
    while current != *top {
        let Some(parent) = current.parent() else { return false };
        if parent.kind() != "parenthesized_expression" && !is_operation(&parent, text, "&&") {
            return false;
        }
        current = parent;
    }
    true
}

/// The variable name an `instanceof` tests, if its operand is a plain name.
fn tested_variable<'a>(instanceof: &Node, text: &'a str) -> Option<&'a str> {
    let operand = strip_parentheses(&instanceof.child_by_field_name("left")?);
    (operand.kind() == "identifier").then(|| source(&operand, text))
}

/// The outermost `&&`/`||` operation the node is an operand of, through parentheses and other `&&`/`||`
/// operations; the node itself if it is not an operand of one.
fn outermost_condition<'t>(node: &Node<'t>, text: &str) -> Node<'t> {
    let mut top = *node;
    let mut current = *node;
    while let Some(parent) = current.parent() {
        if is_operation(&parent, text, "&&") || is_operation(&parent, text, "||") {
            top = parent;
        } else if parent.kind() != "parenthesized_expression" {
            break;
        }
        current = parent;
    }
    top
}

/// Whether the node is `name != null` or `null != name`.
fn is_null_check(node: &Node, name: &str, text: &str) -> bool {
    if !is_operation(node, text, "!=") {
        return false;
    }
    let is_name = |n: &Node| n.kind() == "identifier" && source(n, text) == name;
    match operands(node) {
        [Some(left), Some(right)] => {
            (is_name(&left) && right.kind() == "null_literal") || (left.kind() == "null_literal" && is_name(&right))
        }
        _ => false,
    }
}

fn is_operation(node: &Node, text: &str, op: &str) -> bool {
    node.kind() == "binary_expression" && operator(node, text) == op
}

fn operands<'t>(operation: &Node<'t>) -> [Option<Node<'t>>; 2] {
    [operation.child_by_field_name("left"), operation.child_by_field_name("right")]
}

/// The edit that removes the null check from its `&&` operation, if the `instanceof` is next to it in the `&&`
/// chain.
fn removal(top: &Node, null_check: &Node, instanceof: &Node, text: &str) -> Option<Edit> {
    let leaves = flatten(top, text);
    let index = leaves.iter().position(|(leaf, _)| strip_parentheses(leaf) == *null_check)?;
    let is_neighbour =
        |i: Option<usize>| i.and_then(|i| leaves.get(i)).is_some_and(|(l, _)| strip_parentheses(l) == *instanceof);
    if !is_neighbour(index.checked_sub(1)) && !is_neighbour(Some(index + 1)) {
        return None;
    }
    let (operand, and) = leaves[index];
    let [Some(left), Some(right)] = operands(&and) else { return None };
    let range = if left == operand { left.start_byte()..right.start_byte() } else { left.end_byte()..right.end_byte() };
    if comment_in(&and, range.clone()) {
        return None;
    }
    Some(Edit::new(range, ""))
}

/// The operands of a chain of `&&` operations (through parentheses) in order, each with the operation it is an
/// operand of.
fn flatten<'t>(and: &Node<'t>, text: &str) -> Vec<(Node<'t>, Node<'t>)> {
    let mut leaves = Vec::new();
    // Operands still to visit with their operation, the next one last.
    let mut pending: Vec<(Node, Node)> = operands(and).into_iter().flatten().rev().map(|o| (o, *and)).collect();
    while let Some((operand, operation)) = pending.pop() {
        let inner = strip_parentheses(&operand);
        if is_operation(&inner, text, "&&") {
            pending.extend(operands(&inner).into_iter().flatten().rev().map(|o| (o, inner)));
        } else {
            leaves.push((operand, operation));
        }
    }
    leaves
}
