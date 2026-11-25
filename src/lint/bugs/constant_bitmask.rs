//! B086: a bitwise expression whose result is constant (NetBeans Incompatible mask, IntelliJ IncompatibleMask, Error
//! Prone ErroneousBitwiseExpression):
//! - `(x & c1) == c2` where `c2` has a bit that `c1` lacks: always false (`!=` always true);
//! - `(x | c1) == c2` where `c1` has a bit that `c2` lacks: always false (`!=` always true);
//! - `x & c1 & c2` and `c1 & c2` where `c1 & c2` is 0: always 0, likely flags meant to be combined with `|`.
//!
//! No fix: what was meant is unclear. Only integer literals and constant expressions of them count as constants
//! (the originals also resolve named constants). Values are compared after Java's widening of `int` to `long`,
//! which keeps the sign: the bits a masked `int` can have are those of the widened mask.

use super::shared_arithmetic::{Value, constant};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Binary { op, lhs, rhs } = &tree.kind else { return };
    let text = context.text;
    let message = match op.as_str() {
        "==" | "!=" => {
            let (masked, compared) = match (constant(lhs, text), constant(rhs, text)) {
                (None, Some(c)) => (lhs, c.wide()),
                (Some(c), None) => (rhs, c.wide()),
                _ => return,
            };
            let Some((mask_op, mask)) = masked_with(masked, context) else { return };
            let incompatible = match mask_op {
                "&" => compared & !mask != 0,
                _ => mask & !compared != 0,
            };
            if !incompatible {
                return;
            }
            format!("Incompatible bit mask: the comparison is always {}", op == "!=")
        }
        "&" => {
            let both_constant = |a: Option<Value>, b: Option<Value>| match (a, b) {
                (Some(a), Some(b)) => Some((a.wide(), b.wide())),
                _ => None,
            };
            let masks = both_constant(constant(lhs, text), constant(rhs, text))
                .or_else(|| chained_masks(lhs, rhs, context))
                .or_else(|| chained_masks(rhs, lhs, context));
            match masks {
                Some((a, b)) if a != 0 && b != 0 && a & b == 0 => {
                    "Bitwise & of masks without common bits is always 0".to_string()
                }
                _ => return,
            }
        }
        _ => return,
    };
    diagnostics.push(Diagnostic::new(Rule::ConstantBitmask, tree.start, message));
}

/// The operator and constant of `x & c` or `x | c` (in either order), where `x` is no constant.
fn masked_with<'a>(expression: &'a Tree, context: &Context) -> Option<(&'a str, i64)> {
    let K::Binary { op, lhs, rhs } = &strip_parens(expression).kind else { return None };
    if op != "&" && op != "|" {
        return None;
    }
    match (constant(lhs, context.text), constant(rhs, context.text)) {
        (None, Some(c)) | (Some(c), None) => Some((op, c.wide())),
        _ => None,
    }
}

/// The masks of `(x & c1) & c2`, with `inner` as `x & c1` and `outer` as `c2`.
fn chained_masks(inner: &Tree, outer: &Tree, context: &Context) -> Option<(i64, i64)> {
    let outer = constant(outer, context.text)?.wide();
    match masked_with(inner, context)? {
        ("&", inner) => Some((inner, outer)),
        _ => None,
    }
}
