//! B017: `new BigDecimal(0.1)` (PMD AvoidDecimalLiteralsInBigDecimalConstructor): the double literal is not exactly
//! 0.1, so neither is the BigDecimal.
//!
//! Like PMD, which reports every call of the `BigDecimal(double)` constructor, also arguments that are not literals
//! but whose type is known to be `double` or `float` (or their wrappers) without type resolution: variables declared
//! with these types in the enclosing code, casts, arithmetic with such an operand, and calls of `doubleValue()`,
//! `floatValue()`, `Double.parseDouble` and the `Math` functions that only return `double`.

use super::shared_b::declared_type;
use crate::lint::tree::{Context, path_to, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

pub fn check<'t>(tree: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let K::NewClass { ident, args, .. } = &tree.kind else { return };
    if !matches!(context.source(ident), "BigDecimal" | "java.math.BigDecimal") {
        return;
    }
    let floating = Floating { creation: tree, context };
    if matches!(args.as_slice(), [argument] if floating.is_value(argument)) {
        let message = "BigDecimal created from a double; use a String or BigDecimal.valueOf";
        diagnostics.push(Diagnostic::new(Rule::BigDecimalDouble, tree.start, message));
    }
}

/// Finds out whether expressions in the arguments of a creation are floating.
struct Floating<'a, 't> {
    creation: &'t Tree,
    context: &'a Context<'t>,
}

impl Floating<'_, '_> {
    /// Whether the expression's type is known to be `double` or `float`, or a wrapper of them.
    fn is_value(&self, expression: &Tree) -> bool {
        let text = self.context.text;
        let expression = strip_parens(expression);
        match &expression.kind {
            K::Literal(LitKind::Float | LitKind::Double) => true,
            K::Unary { op, expr, .. } if op == "-" || op == "+" => self.is_value(expr),
            K::Cast { ty, .. } => is_floating_type(&text[ty.start..ty.end]),
            K::Ident(name) => self.declared_type(expression, name).is_some_and(is_floating_type),
            K::Binary { op, lhs, rhs } => match op.as_str() {
                // Both operands are numbers, so one floating operand makes the result floating.
                "-" | "*" | "/" | "%" => self.is_value(lhs) || self.is_value(rhs),
                // `+` may concatenate strings.
                "+" => self.is_value(lhs) && self.is_number(rhs) || self.is_number(lhs) && self.is_value(rhs),
                _ => false,
            },
            K::MethodInvocation { select, .. } => {
                let (name, object) = match &select.kind {
                    K::Ident(name) => (name.as_str(), None),
                    K::MemberSelect { expr, name } => (name.as_str(), Some(&text[expr.start..expr.end])),
                    _ => return false,
                };
                matches!(name, "doubleValue" | "floatValue")
                    || matches!((object, name), (Some("Double"), "parseDouble") | (Some("Float"), "parseFloat"))
                    || object == Some("Math") && MATH_DOUBLE_FUNCTIONS.contains(&name)
            }
            _ => false,
        }
    }

    /// Whether the expression is known to be a number: a numeric literal or a floating value. Like tree-sitter's
    /// integer literals, a negative integer literal is not a literal.
    fn is_number(&self, expression: &Tree) -> bool {
        let expression = strip_parens(expression);
        let is_integer = matches!(expression.kind, K::Literal(LitKind::Int | LitKind::Long))
            && !self.context.source(expression).starts_with('-');
        is_integer || self.is_value(expression)
    }

    /// The declared type of the variable that the identifier in the creation names.
    fn declared_type(&self, identifier: &Tree, name: &str) -> Option<&str> {
        let mut path = self.context.ancestors.clone();
        path.push(self.creation);
        path.extend(path_to(self.creation, identifier.start..identifier.end));
        declared_type(&path, name, self.context.text)
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

fn is_floating_type(declared: &str) -> bool {
    matches!(declared, "double" | "float" | "Double" | "Float" | "java.lang.Double" | "java.lang.Float")
}
