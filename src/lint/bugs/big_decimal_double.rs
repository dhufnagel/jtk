//! B017: `new BigDecimal(0.1)` (PMD AvoidDecimalLiteralsInBigDecimalConstructor): the double literal is not exactly
//! 0.1, so neither is the BigDecimal.
//!
//! Like PMD, which reports every call of the `BigDecimal(double)` constructor, also arguments that are not literals
//! but whose type is known to be `double` or `float` (or their wrappers) without type resolution: variables declared
//! with these types in the enclosing code, casts, arithmetic with such an operand, and calls of `doubleValue()`,
//! `floatValue()`, `Double.parseDouble` and the `Math` functions that only return `double`.

use tree_sitter::Node;

use crate::lint::syntax::{declared_type, named_children, operator, source, strip_parentheses};
use crate::lint::{Diagnostic, Rule};

pub fn check(creation: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let is_big_decimal = creation
        .child_by_field_name("type")
        .is_some_and(|t| matches!(source(&t, text), "BigDecimal" | "java.math.BigDecimal"));
    if !is_big_decimal {
        return;
    }
    let arguments = creation.child_by_field_name("arguments").map(|a| named_children(&a)).unwrap_or_default();
    if matches!(arguments.as_slice(), [argument] if is_floating_value(argument, text)) {
        let message = "BigDecimal created from a double; use a String or BigDecimal.valueOf";
        diagnostics.push(Diagnostic::new(Rule::BigDecimalDouble, creation.start_byte(), message));
    }
}

/// Whether the expression's type is known to be `double` or `float`, or a wrapper of them.
fn is_floating_value(expression: &Node, text: &str) -> bool {
    let expression = strip_parentheses(expression);
    match expression.kind() {
        _ if is_floating_literal(&expression, text) => true,
        "hex_floating_point_literal" => true,
        "unary_expression" => {
            matches!(operator(&expression, text), "-" | "+")
                && expression.child_by_field_name("operand").is_some_and(|o| is_floating_value(&o, text))
        }
        "cast_expression" => expression.child_by_field_name("type").is_some_and(|t| is_floating_type(&t, text)),
        "identifier" => {
            declared_type(&expression, source(&expression, text), text).is_some_and(|t| is_floating_type(&t, text))
        }
        "binary_expression" => {
            let (Some(left), Some(right)) =
                (expression.child_by_field_name("left"), expression.child_by_field_name("right"))
            else {
                return false;
            };
            match operator(&expression, text) {
                // Both operands are numbers, so one floating operand makes the result floating.
                "-" | "*" | "/" | "%" => is_floating_value(&left, text) || is_floating_value(&right, text),
                // `+` may concatenate strings.
                "+" => {
                    is_floating_value(&left, text) && is_number(&right, text)
                        || is_number(&left, text) && is_floating_value(&right, text)
                }
                _ => false,
            }
        }
        "method_invocation" => {
            let name = expression.child_by_field_name("name").map_or("", |n| source(&n, text));
            let object = expression.child_by_field_name("object").map(|o| source(&o, text));
            matches!(name, "doubleValue" | "floatValue")
                || matches!((object, name), (Some("Double"), "parseDouble") | (Some("Float"), "parseFloat"))
                || object == Some("Math") && MATH_DOUBLE_FUNCTIONS.contains(&name)
        }
        _ => false,
    }
}

/// The `Math` methods that return `double` and have no overloads for other types.
const MATH_DOUBLE_FUNCTIONS: &[&str] = &[
    "random",
    "sqrt",
    "cbrt",
    "pow",
    "exp",
    "expm1",
    "log",
    "log10",
    "log1p",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    "atan2",
    "sinh",
    "cosh",
    "tanh",
    "hypot",
    "floor",
    "ceil",
    "rint",
    "toRadians",
    "toDegrees",
];

/// Whether the expression is known to be a number: a numeric literal or a floating value.
fn is_number(expression: &Node, text: &str) -> bool {
    let expression = strip_parentheses(expression);
    matches!(
        expression.kind(),
        "decimal_integer_literal" | "hex_integer_literal" | "octal_integer_literal" | "binary_integer_literal"
    ) || is_floating_value(&expression, text)
}

fn is_floating_type(declared: &Node, text: &str) -> bool {
    matches!(source(declared, text), "double" | "float" | "Double" | "Float" | "java.lang.Double" | "java.lang.Float")
}

fn is_floating_literal(argument: &Node, text: &str) -> bool {
    match argument.kind() {
        "decimal_floating_point_literal" => true,
        "unary_expression" if matches!(operator(argument, text), "-" | "+") => {
            argument.child_by_field_name("operand").is_some_and(|o| is_floating_literal(&o, text))
        }
        _ => false,
    }
}
