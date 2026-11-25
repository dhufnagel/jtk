//! B108: `^` used as a power operator, as in `2 ^ 8` or `10 ^ 3` (Error Prone XorPower): `^` is the bitwise
//! exclusive or, so `2 ^ 8` is 10, not 256.
//!
//! Fix, like Error Prone's: `1 << 8` for powers of 2 up to `1 << 31` (`1L << 63` for a `long` base), and `1000` for
//! powers of 10 up to 10^9 (10^18 for a `long` base). It is marked safe like in the catalogue: the exclusive or of
//! these literals is never what was meant. Where the base is the right operand of an arithmetic operator
//! (`a * 2 ^ 8`, which is `(a * 2) ^ 8`), the power replaces the base and the exponent, in parentheses:
//! `a * (1 << 8)`.
//!
//! Like Error Prone, the base must be the decimal literal 2 or 10 and the exponent a decimal integer literal, not a
//! hexadecimal or octal one (`2 ^ 0x10` is more likely a real exclusive or).

use super::shared_arithmetic::{Value, integer_literal};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Binary { op, lhs, rhs } = &tree.kind else { return };
    if op != "^" {
        return;
    }
    let text = context.text;
    // The base is the operand next to `^`: the rightmost one of a tighter operation on the left.
    let mut base = &**lhs;
    while let K::Binary { op, rhs, .. } = &base.kind {
        if !matches!(op.as_str(), "*" | "/" | "%" | "+" | "-") {
            return;
        }
        base = rhs;
    }
    let base_value = match integer_literal(base, text) {
        Some(value @ (Value::Int(2 | 10) | Value::Long(2 | 10))) => value,
        _ => return,
    };
    let is_decimal = |t: &Tree| !context.source(t).starts_with('0');
    let Some(exponent) = integer_literal(rhs, text).map(Value::wide) else { return };
    if !is_decimal(base) || !is_decimal(rhs) || !(0..=i64::from(i32::MAX)).contains(&exponent) {
        return;
    }
    let is_long = matches!(base_value, Value::Long(_));
    let suffix = if is_long { "L" } else { "" };
    let value = base_value.wide() ^ exponent;
    let message = format!("^ is exclusive or, not a power: {} ^ {exponent} is {value}", base_value.wide());
    let diagnostic = Diagnostic::new(Rule::XorPower, tree.start, message);
    let power = match base_value.wide() {
        2 if exponent <= if is_long { 63 } else { 31 } => Some(format!("1{suffix} << {exponent}")),
        10 if exponent <= if is_long { 18 } else { 9 } => Some(format!("1{}{suffix}", "0".repeat(exponent as usize))),
        _ => None,
    };
    let range = base.start..tree.end;
    diagnostics.push(match power {
        Some(power) if !context.tokens().has_comment(range.clone()) => {
            let in_operation = base.start != tree.start && power.contains("<<");
            let replacement = if in_operation { format!("({power})") } else { power };
            diagnostic.with_fix(vec![Edit::new(range, replacement)])
        }
        _ => diagnostic,
    });
}
