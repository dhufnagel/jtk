//! B016: null checks that come too late or test the wrong way (PMD MisplacedNullCheck, BrokenNullCheck):
//! `x.foo() && x != null` dereferences before checking, `x != null || x.foo()` dereferences exactly when `x` is null.
//!
//! Like PMD, the checked expression may be a variable, a field or a getter call (`a.getB()`), and besides calls,
//! field and array accesses, method references (`x::foo`), qualified instance creations (`x.new Inner()`) and
//! unboxing (`x == 0`, `x < 1`, `x && b`) count as uses that throw. A use guarded by its own null check of the same
//! expression (`(x != null && x.foo()) || x == null`) is not reported. Deviations: jtk reports the null check, not
//! the use, and checks every operand of a chain of `&&` or `||`, while PMD's BrokenNullCheck only looks at the
//! operand right after the check. `x + 1` only counts as unboxing if `x` is declared with a wrapper type, as it may
//! concatenate strings.

use tree_sitter::Node;

use crate::lint::syntax::{declared_type, for_each_descendant, operator, source, strip_parentheses};
use crate::lint::{Diagnostic, Rule};

pub fn check(binary: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let chain_operator = operator(binary, text);
    if !matches!(chain_operator, "&&" | "||") {
        return;
    }
    let is_chain_top =
        binary.parent().is_none_or(|p| p.kind() != "binary_expression" || operator(&p, text) != chain_operator);
    if !is_chain_top {
        return;
    }
    let operands = flatten(binary, chain_operator, text);
    // In `a && b`, b is evaluated when a is true; in `a || b`, when a is false.
    let continues_when_null = |is_null_test: bool| (chain_operator == "&&") == is_null_test;
    for (i, operand) in operands.iter().enumerate() {
        let Some((checked, is_null_test)) = null_test(operand, text) else { continue };
        let checked_text = source(&checked, text);
        let throws = |o: &Node| throws_if_null(o, &checked, checked_text, text);
        let message = if operands[..i].iter().any(throws) {
            format!("'{checked_text}' is dereferenced before it is checked for null")
        } else if continues_when_null(is_null_test) && operands[i + 1..].iter().any(throws) {
            format!("'{checked_text}' is dereferenced exactly when it is null")
        } else {
            continue;
        };
        diagnostics.push(Diagnostic::new(Rule::NullCheck, operand.start_byte(), message));
    }
}

fn flatten<'t>(node: &Node<'t>, chain_operator: &str, text: &str) -> Vec<Node<'t>> {
    let node = strip_parentheses(node);
    if node.kind() == "binary_expression" && operator(&node, text) == chain_operator {
        let mut operands = Vec::new();
        for side in ["left", "right"] {
            if let Some(operand) = node.child_by_field_name(side) {
                operands.extend(flatten(&operand, chain_operator, text));
            }
        }
        return operands;
    }
    vec![node]
}

/// `x == null` or `x != null` (either way round): the tested expression, and whether the test is `== null`.
fn null_test<'t>(operand: &Node<'t>, text: &str) -> Option<(Node<'t>, bool)> {
    if operand.kind() != "binary_expression" {
        return None;
    }
    let is_null_test = match operator(operand, text) {
        "==" => true,
        "!=" => false,
        _ => return None,
    };
    let left = strip_parentheses(&operand.child_by_field_name("left")?);
    let right = strip_parentheses(&operand.child_by_field_name("right")?);
    let checked = match (left.kind(), right.kind()) {
        (_, "null_literal") => left,
        ("null_literal", _) => right,
        _ => return None,
    };
    (is_stable_path(&checked, text) && checked.kind() != "this").then_some((checked, is_null_test))
}

/// A variable, or a chain of field accesses and getter calls on one: an expression that gives the same value when
/// evaluated twice in a row (PMD's `StablePathMatcher`).
fn is_stable_path(expression: &Node, text: &str) -> bool {
    match expression.kind() {
        "identifier" | "this" => true,
        "field_access" => expression.child_by_field_name("object").is_some_and(|o| is_stable_path(&o, text)),
        "method_invocation" => {
            let name = expression.child_by_field_name("name").map_or("", |n| source(&n, text));
            let is_getter = ["get", "is"].iter().any(|prefix| {
                name.strip_prefix(prefix).and_then(|rest| rest.chars().next()).is_some_and(char::is_uppercase)
            });
            is_getter
                && expression.child_by_field_name("arguments").is_some_and(|a| a.named_child_count() == 0)
                && expression.child_by_field_name("object").is_none_or(|o| is_stable_path(&o, text))
        }
        _ => false,
    }
}

/// Whether evaluating the operand throws a NullPointerException if `checked` is null: it dereferences or unboxes
/// `checked`, and does not check it for null itself.
fn throws_if_null(operand: &Node, checked: &Node, checked_text: &str, text: &str) -> bool {
    let is_checked = |node: Option<Node>| node.is_some_and(|n| source(&strip_parentheses(&n), text) == checked_text);
    let mut throws = is_checked(Some(*operand));
    let mut guarded = false;
    let mut visit = |node: &Node| {
        guarded |= null_test(node, text).is_some_and(|(c, _)| source(&c, text) == checked_text);
        throws |= match node.kind() {
            "method_invocation" | "field_access" => is_checked(node.child_by_field_name("object")),
            "array_access" => is_checked(node.child_by_field_name("array")),
            "method_reference" => is_checked(node.named_child(0)),
            "object_creation_expression" => {
                node.child(0)
                    .is_some_and(|c| c.is_named() && c.kind() != "annotation" && c.kind() != "marker_annotation")
                    && is_checked(node.child(0))
            }
            "unary_expression" => {
                matches!(operator(node, text), "!" | "-" | "+" | "~") && is_checked(node.child_by_field_name("operand"))
            }
            "binary_expression" => unboxes(node, checked, &is_checked, text),
            _ => false,
        };
    };
    visit(operand);
    for_each_descendant(operand, &["lambda_expression", "class_body"], visit);
    throws && !guarded
}

/// Whether the binary expression unboxes an operand that is `checked`.
fn unboxes(binary: &Node, checked: &Node, is_checked: &dyn Fn(Option<Node>) -> bool, text: &str) -> bool {
    let (left, right) = (binary.child_by_field_name("left"), binary.child_by_field_name("right"));
    let checked_side = if is_checked(left) {
        right
    } else if is_checked(right) {
        left
    } else {
        return false;
    };
    match operator(binary, text) {
        "==" | "!=" => checked_side.is_some_and(|other| is_primitive_literal(&strip_parentheses(&other))),
        "+" => is_wrapper_variable(checked, text),
        _ => true,
    }
}

fn is_primitive_literal(expression: &Node) -> bool {
    matches!(
        expression.kind(),
        "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal"
            | "decimal_floating_point_literal"
            | "hex_floating_point_literal"
            | "character_literal"
            | "true"
            | "false"
    )
}

/// Whether `checked` is a variable declared with a numeric wrapper type, which `+` unboxes.
fn is_wrapper_variable(checked: &Node, text: &str) -> bool {
    checked.kind() == "identifier"
        && declared_type(checked, source(checked, text), text).is_some_and(|t| {
            matches!(source(&t, text), "Integer" | "Long" | "Short" | "Byte" | "Character" | "Double" | "Float")
        })
}
