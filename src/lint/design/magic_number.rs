//! A015: a numeric literal outside the definition of a constant, other than those in `ignore-numbers` (-1, 0, 1 and
//! 2 by default; Checkstyle MagicNumber with its other properties' defaults).
//!
//! As in Checkstyle, a constant definition is a `final` field or local variable, an interface or annotation field,
//! or an enum constant's arguments, and a number in it only counts as a constant if it is combined with arithmetic,
//! comparison, bitwise or conditional operators, casts, array initializers and calls; it is still magic when used
//! as an array size, an array index, in a lambda, after `&&` or `||` and the like. Numbers in annotation element
//! defaults are not checked. Values are compared as Checkstyle parses them: `0xffffffff` is -1, and -0.0 is magic.
//!
//! Unlike Checkstyle without its suppression filter, jtk honours `@SuppressWarnings("MagicNumber")`.

use tree_sitter::Node;

use super::{TYPE_DECLARATIONS, has_keyword, walk};
use crate::lint::syntax::{operator, source};
use crate::lint::{Diagnostic, Rule, RuleOptions};

pub fn check(program: &Node, text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let ignored = &options.magic_number.ignore_numbers;
    let mut ancestors: Vec<Node> = Vec::new();
    walk(program, |node, enter| {
        if !enter {
            ancestors.pop();
            return;
        }
        if is_number(node) {
            check_number(node, &ancestors, ignored, text, diagnostics);
        }
        ancestors.push(*node);
    });
}

fn is_number(node: &Node) -> bool {
    matches!(
        node.kind(),
        "decimal_integer_literal"
            | "hex_integer_literal"
            | "octal_integer_literal"
            | "binary_integer_literal"
            | "decimal_floating_point_literal"
            | "hex_floating_point_literal"
    )
}

fn check_number(number: &Node, ancestors: &[Node], ignored: &[f64], text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if ancestors.iter().any(|a| a.kind() == "annotation_type_element_declaration") {
        return;
    }
    // The sign before the number, outside any parentheses.
    let sign = ancestors
        .iter()
        .rev()
        .find(|a| a.kind() != "parenthesized_expression")
        .filter(|a| a.kind() == "unary_expression" && matches!(operator(a, text), "-" | "+"));
    let is_negative = sign.is_some_and(|s| operator(s, text) == "-");
    let value = value(number, text).map(|v| if is_negative { -v } else { v });
    // Compared like Java's `Arrays.binarySearch`, for which -0.0 is not 0.
    let is_ignored = value.is_some_and(|v| ignored.iter().any(|n| n.total_cmp(&v).is_eq()));
    if is_ignored || !is_magic(number, ancestors, text) {
        return;
    }
    let (offset, prefix) = sign.map_or((number.start_byte(), ""), |s| (s.start_byte(), operator(s, text)));
    let message = format!("'{prefix}{}' is a magic number", source(number, text));
    diagnostics.push(Diagnostic::new(Rule::MagicNumber, offset, message));
}

/// Whether a number with these ancestors is not part of a constant definition.
fn is_magic(number: &Node, ancestors: &[Node], text: &str) -> bool {
    // The nearest definition of a variable or enum constant.
    let Some(definition) = ancestors.iter().rposition(|a| matches!(a.kind(), "enum_constant" | "variable_declarator"))
    else {
        return true;
    };
    if ancestors[definition].kind() == "variable_declarator" {
        let declaration = &ancestors[definition - 1];
        let is_constant =
            matches!(declaration.kind(), "field_declaration" | "local_variable_declaration" | "constant_declaration")
                && (is_in_interface(&ancestors[..definition - 1]) || has_keyword(declaration, text, "final"));
        if !is_constant {
            return true;
        }
    }
    // Whether some expression between the definition and the number is not one constants may be built with.
    (definition + 1..ancestors.len()).any(|i| {
        let child = ancestors.get(i + 1).unwrap_or(number);
        !is_constant_expression(&ancestors[i], child, text)
    })
}

/// Whether the nearest enclosing type of a declaration is an interface or annotation type, as Checkstyle sees it:
/// it stops at any `new` expression.
fn is_in_interface(ancestors: &[Node]) -> bool {
    ancestors
        .iter()
        .rev()
        .find(|a| {
            TYPE_DECLARATIONS.contains(&a.kind())
                || matches!(a.kind(), "object_creation_expression" | "array_creation_expression")
        })
        .is_some_and(|a| matches!(a.kind(), "interface_declaration" | "annotation_type_declaration"))
}

/// Whether a constant may contain an expression of the kind of `node` around its `child` (Checkstyle's
/// `constantWaiverParentToken`).
fn is_constant_expression(node: &Node, child: &Node, text: &str) -> bool {
    match node.kind() {
        "parenthesized_expression"
        | "cast_expression"
        | "ternary_expression"
        | "argument_list"
        | "array_initializer"
        | "object_creation_expression"
        | "array_creation_expression" => true,
        "binary_expression" => !matches!(operator(node, text), "&&" | "||"),
        "unary_expression" => operator(node, text) != "!",
        "assignment_expression" => operator(node, text) == "=",
        "method_invocation" => child.kind() == "argument_list",
        _ => false,
    }
}

/// The value of a numeric literal, as Checkstyle parses it.
fn value(number: &Node, text: &str) -> Option<f64> {
    let literal = source(number, text).replace('_', "");
    match number.kind() {
        "decimal_floating_point_literal" => literal.trim_end_matches(['f', 'F', 'd', 'D']).parse().ok(),
        "hex_floating_point_literal" => None,
        _ => {
            let is_long = literal.ends_with(['l', 'L']);
            let literal = literal.trim_end_matches(['l', 'L']);
            let (digits, radix) = if let Some(hex) = literal.strip_prefix("0x").or_else(|| literal.strip_prefix("0X")) {
                (hex, 16)
            } else if let Some(binary) = literal.strip_prefix("0b").or_else(|| literal.strip_prefix("0B")) {
                (binary, 2)
            } else if literal.len() > 1 && literal.starts_with('0') {
                (&literal[1..], 8)
            } else {
                (literal, 10)
            };
            // Like Java's `parseUnsignedInt` and `parseUnsignedLong`: `0xffffffff` is -1.
            let value = u64::from_str_radix(digits, radix).ok()?;
            Some(if is_long { value as i64 as f64 } else { value as u32 as i32 as f64 })
        }
    }
}
