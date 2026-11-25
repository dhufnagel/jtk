//! S070: a negated comparison or a double negation (IntelliJ NegatedEqualityExpression and DoubleNegation, PMD
//! LogicInversion, Picnic's EqualityRules and PrimitiveRules): `!(a == b)` is `a != b`, `!!b` is `b`.
//!
//! The fix writes the comparison with the opposite operator, or drops both negations. `!(a < b)` is only reported
//! for operands that are certainly integral (integer and character literals, variables declared with an integral or
//! its boxed type, arithmetic on those): with a floating point NaN, `!(a < b)` is true but `a >= b` is false. The
//! comparison gets parentheses where the negation was an operand of an operator that binds tighter than `==` (`c ==
//! !(a == b)` becomes `c == (a != b)`). A comparison with a boolean literal is left to S003, which removes the literal.

use crate::lint::tree::{Context, is_boolean_literal, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

use super::shared_batch3::{class_name, declared_type, refers_to};

const INTEGRAL: &[&str] = &["int", "long", "short", "byte", "char"];
const BOXED_INTEGRAL: &[&str] = &["Integer", "Long", "Short", "Byte", "Character"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if !is_negation(tree) {
        return;
    }
    // Of `!!!b`, the outermost negation is reported.
    let outer = context.ancestors.iter().rev().find(|a| !matches!(a.kind, K::Parens(_)));
    if outer.is_some_and(|o| is_negation(o)) {
        return;
    }
    let K::Unary { expr, .. } = &tree.kind else { return };
    let operand = strip_parens(expr);
    let (message, replacement) = match &operand.kind {
        K::Unary { expr: inner, .. } if is_negation(operand) && !is_boolean_literal(strip_parens(inner)) => {
            ("Double negation", context.source(inner).to_string())
        }
        K::Binary { op, lhs, rhs } => {
            let Some(inverse) = inverse(op) else { return };
            let literal = is_boolean_literal(strip_parens(lhs)) || is_boolean_literal(strip_parens(rhs));
            let relational = !matches!(op.as_str(), "==" | "!=");
            if literal || relational && !(is_integral(lhs, tree, context) && is_integral(rhs, tree, context)) {
                return;
            }
            let comparison = format!("{} {inverse} {}", context.source(lhs), context.source(rhs));
            let comparison = if needs_parentheses(context.parent()) { format!("({comparison})") } else { comparison };
            ("Negated comparison can be simplified", comparison)
        }
        _ => return,
    };
    let diagnostic = Diagnostic::new(Rule::NegatedComparison, tree.start, message);
    diagnostics.push(if context.tokens().has_comment(tree.start..tree.end) {
        diagnostic
    } else {
        diagnostic.with_fix(vec![Edit::new(tree.start..tree.end, replacement)])
    });
}

fn is_negation(tree: &Tree) -> bool {
    matches!(&tree.kind, K::Unary { op, .. } if op == "!")
}

fn inverse(op: &str) -> Option<&'static str> {
    Some(match op {
        "==" => "!=",
        "!=" => "==",
        "<" => ">=",
        "<=" => ">",
        ">" => "<=",
        ">=" => "<",
        _ => return None,
    })
}

/// Whether a comparison in place of a negation needs parentheses: as operand of an operator that binds tighter
/// than the logical ones.
fn needs_parentheses(parent: Option<&Tree>) -> bool {
    match parent.map(|p| &p.kind) {
        Some(K::Binary { op, .. }) => !matches!(op.as_str(), "&&" | "||" | "&" | "|" | "^"),
        Some(
            K::Unary { .. } | K::Cast { .. } | K::InstanceOf { .. } | K::MemberSelect { .. } | K::ArrayAccess { .. },
        ) => true,
        _ => false,
    }
}

/// Whether the expression is certainly of an integral type (or its boxed type), so never NaN. `scope` is the tree
/// the expression is in, whose ancestors are the context's.
fn is_integral(expression: &Tree, scope: &Tree, context: &Context) -> bool {
    match &expression.kind {
        K::Literal(kind) => matches!(kind, LitKind::Int | LitKind::Long | LitKind::Char),
        K::Parens(inner) => is_integral(inner, scope, context),
        K::Unary { op, expr, .. } => op != "!" && is_integral(expr, scope, context),
        K::Binary { op, lhs, rhs } => {
            let arithmetic = matches!(op.as_str(), "+" | "-" | "*" | "/" | "%" | "&" | "|" | "^" | "<<" | ">>" | ">>>");
            arithmetic && is_integral(lhs, scope, context) && is_integral(rhs, scope, context)
        }
        K::Cast { ty, .. } => matches!(&ty.kind, K::Primitive(p) if INTEGRAL.contains(&p.as_str())),
        K::Ident(name) => {
            let Some(ty) = declared_type(scope, &context.ancestors, name, context) else { return false };
            match &ty.kind {
                K::Primitive(p) => INTEGRAL.contains(&p.as_str()),
                _ => class_name(ty, context).is_some_and(|class| {
                    BOXED_INTEGRAL
                        .iter()
                        .any(|b| refers_to(&class, &format!("java.lang.{b}"), &context.ancestors, context))
                }),
            }
        }
        _ => false,
    }
}
