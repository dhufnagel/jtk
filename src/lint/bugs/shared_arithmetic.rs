//! Constant folding of integer expressions built from literals, with Java's `int` and `long` semantics, for the rules
//! on arithmetic with constant or suspicious results.

use jtk_parser::ast::{K, LitKind, Tree};

use super::shared_b::declared_type;
use crate::lint::tree::{Context, path_to, strip_parens};

/// The value of an integer constant expression, with its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Value {
    Int(i32),
    Long(i64),
}

impl Value {
    /// The value, widened to `long` as Java's numeric promotion does.
    pub fn wide(self) -> i64 {
        match self {
            Value::Int(v) => i64::from(v),
            Value::Long(v) => v,
        }
    }
}

/// The result of folding an expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Folded {
    /// The value Java computes, wrapped on overflow. `None` if the expression is no integer constant built from
    /// literals, divides by zero or overflows where that looks intended (`0x7fffffff + 1`).
    pub value: Option<Value>,
    /// Whether an operation of the expression overflowed.
    pub overflowed: bool,
}

/// The value of the integer constant expression `tree`, made of integer literals, parentheses, unary and binary
/// arithmetic and bitwise operators and casts to integral types. Names of constants are not resolved.
pub(super) fn constant(tree: &Tree, text: &str) -> Option<Value> {
    fold(tree, text).value
}

/// Folds `tree` (see [`constant`]), noting overflows.
pub(super) fn fold(tree: &Tree, text: &str) -> Folded {
    fold_with(tree, text, None)
}

/// Folds `tree` as if the literal `as_long` were a `long` literal.
pub(super) fn fold_with(tree: &Tree, text: &str, as_long: Option<&Tree>) -> Folded {
    let mut folder = Folder { text, as_long, overflowed: false };
    let value = folder.fold(tree, 0);
    Folded { value, overflowed: folder.overflowed }
}

/// The value of an `int` or `long` literal (including a `-` that javac counts as part of it).
pub(super) fn integer_literal(tree: &Tree, text: &str) -> Option<Value> {
    let K::Literal(kind @ (LitKind::Int | LitKind::Long)) = tree.kind else { return None };
    let source = &text[tree.start..tree.end];
    let (negative, digits) = match source.strip_prefix('-') {
        Some(rest) => (true, rest.trim_start()),
        None => (false, source),
    };
    let digits: String = digits.chars().filter(|&c| c != '_' && c != 'l' && c != 'L').collect();
    let lower = digits.to_ascii_lowercase();
    let (radix, digits) = if let Some(hex) = lower.strip_prefix("0x") {
        (16, hex)
    } else if let Some(binary) = lower.strip_prefix("0b") {
        (2, binary)
    } else if lower.len() > 1 && lower.starts_with('0') {
        (8, &lower[1..])
    } else {
        (10, lower.as_str())
    };
    let magnitude = u64::from_str_radix(digits, radix).ok()?;
    let value = match kind {
        // Non-decimal literals give the bits of the value.
        LitKind::Int if radix != 10 => Value::Int(u32::try_from(magnitude).ok()? as i32),
        LitKind::Int => Value::Int(
            i32::try_from(i64::try_from(magnitude).ok()?)
                .ok()
                .or_else(|| (negative && magnitude == 1 << 31).then_some(i32::MIN))?,
        ),
        _ if radix != 10 => Value::Long(magnitude as i64),
        _ => Value::Long(
            i64::try_from(magnitude).ok().or_else(|| (negative && magnitude == 1 << 63).then_some(i64::MIN))?,
        ),
    };
    Some(match (negative, value) {
        (false, value) => value,
        (true, Value::Int(v)) => Value::Int(v.wrapping_neg()),
        (true, Value::Long(v)) => Value::Long(v.wrapping_neg()),
    })
}

struct Folder<'a> {
    text: &'a str,
    as_long: Option<&'a Tree>,
    overflowed: bool,
}

/// Deeper expressions are not folded: the folding is recursive, and long chains of `+` are common.
const MAX_DEPTH: usize = 64;

impl Folder<'_> {
    fn fold(&mut self, tree: &Tree, depth: usize) -> Option<Value> {
        if depth > MAX_DEPTH {
            return None;
        }
        match &tree.kind {
            K::Literal(_) => {
                let value = integer_literal(tree, self.text)?;
                Some(if self.as_long.is_some_and(|t| std::ptr::eq(t, tree)) {
                    Value::Long(value.wide())
                } else {
                    value
                })
            }
            K::Parens(inner) => self.fold(inner, depth + 1),
            K::Unary { op, postfix: false, expr } => {
                let value = self.fold(expr, depth + 1)?;
                match (op.as_str(), value) {
                    ("+", value) => Some(value),
                    ("-", Value::Int(v)) => Some(Value::Int(v.wrapping_neg())),
                    ("-", Value::Long(v)) => Some(Value::Long(v.wrapping_neg())),
                    ("~", Value::Int(v)) => Some(Value::Int(!v)),
                    ("~", Value::Long(v)) => Some(Value::Long(!v)),
                    _ => None,
                }
            }
            K::Cast { ty, expr } => {
                let K::Primitive(ty) = &ty.kind else { return None };
                let value = self.fold(expr, depth + 1)?.wide();
                match ty.as_str() {
                    "long" => Some(Value::Long(value)),
                    "int" => Some(Value::Int(value as i32)),
                    "short" => Some(Value::Int(i32::from(value as i16))),
                    "byte" => Some(Value::Int(i32::from(value as i8))),
                    "char" => Some(Value::Int(i32::from(value as u16))),
                    _ => None,
                }
            }
            K::Binary { op, lhs, rhs } => {
                let lhs = self.fold(lhs, depth + 1)?;
                let rhs = self.fold(rhs, depth + 1)?;
                self.binary(op, lhs, rhs)
            }
            _ => None,
        }
    }

    fn binary(&mut self, op: &str, lhs: Value, rhs: Value) -> Option<Value> {
        // The type of a shift is that of its left operand.
        if matches!(op, "<<" | ">>" | ">>>") {
            let distance = rhs.wide() as u32;
            return Some(match lhs {
                Value::Int(v) => Value::Int(match op {
                    "<<" => v.wrapping_shl(distance),
                    ">>" => v.wrapping_shr(distance),
                    _ => (v as u32).wrapping_shr(distance) as i32,
                }),
                Value::Long(v) => Value::Long(match op {
                    "<<" => v.wrapping_shl(distance),
                    ">>" => v.wrapping_shr(distance),
                    _ => (v as u64).wrapping_shr(distance) as i64,
                }),
            });
        }
        // Like Error Prone, `MAX_VALUE + 1` and `MIN_VALUE - 1` are taken as intended.
        let intended_wrap = match (op, lhs) {
            ("+", Value::Int(v)) => v == i32::MAX,
            ("+", Value::Long(v)) => v == i64::MAX,
            ("-", Value::Int(v)) => v == i32::MIN,
            ("-", Value::Long(v)) => v == i64::MIN,
            _ => false,
        };
        if intended_wrap {
            return None;
        }
        match (lhs, rhs) {
            (Value::Int(a), Value::Int(b)) => {
                let (value, overflowed) = int_operation(op, a, b)?;
                self.overflowed |= overflowed;
                Some(Value::Int(value))
            }
            _ => {
                let (value, overflowed) = long_operation(op, lhs.wide(), rhs.wide())?;
                self.overflowed |= overflowed;
                Some(Value::Long(value))
            }
        }
    }
}

/// The result of an operation on `int`s, wrapped, and whether it overflowed. `None` on division by zero.
fn int_operation(op: &str, a: i32, b: i32) -> Option<(i32, bool)> {
    Some(match op {
        "+" => a.overflowing_add(b),
        "-" => a.overflowing_sub(b),
        "*" => a.overflowing_mul(b),
        "/" if b != 0 => a.overflowing_div(b),
        "%" if b != 0 => (a.wrapping_rem(b), false),
        "&" => (a & b, false),
        "|" => (a | b, false),
        "^" => (a ^ b, false),
        _ => return None,
    })
}

/// The result of an operation on `long`s, wrapped, and whether it overflowed. `None` on division by zero.
fn long_operation(op: &str, a: i64, b: i64) -> Option<(i64, bool)> {
    Some(match op {
        "+" => a.overflowing_add(b),
        "-" => a.overflowing_sub(b),
        "*" => a.overflowing_mul(b),
        "/" if b != 0 => a.overflowing_div(b),
        "%" if b != 0 => (a.wrapping_rem(b), false),
        "&" => (a & b, false),
        "|" => (a | b, false),
        "^" => (a ^ b, false),
        _ => return None,
    })
}

/// Whether `tree` (in parentheses or not) is a call of a method with the name and number of arguments.
pub(super) fn is_call(tree: &Tree, name: &str, arguments: usize) -> bool {
    matches!(&strip_parens(tree).kind, K::MethodInvocation { select, args, .. }
        if args.len() == arguments && method_name(select) == Some(name))
}

/// The name of the method a call's `select` names.
pub(super) fn method_name(select: &Tree) -> Option<&str> {
    match &select.kind {
        K::Ident(name) => Some(name),
        K::MemberSelect { name, .. } => Some(name),
        _ => None,
    }
}

/// The comparison with the operands swapped: `a < b` is `b > a`.
pub(super) fn mirrored(op: &str) -> &str {
    match op {
        "<" => ">",
        ">" => "<",
        "<=" => ">=",
        ">=" => "<=",
        other => other,
    }
}

/// Whether the operator compares.
pub(super) fn is_comparison(op: &str) -> bool {
    matches!(op, "==" | "!=" | "<" | ">" | "<=" | ">=")
}

/// The declared type of the variable `name` (an identifier below `tree`), without type arguments and without the
/// package of `java.lang` and `java.util` types: `List` for `java.util.List<String> list`. `None` if the type is not
/// known in the file (see [`declared_type`]) or is `var`.
pub(super) fn variable_type<'t>(context: &Context<'t>, tree: &'t Tree, name: &'t Tree) -> Option<&'t str> {
    let K::Ident(identifier) = &name.kind else { return None };
    let mut path = context.ancestors.clone();
    path.push(tree);
    path.extend(path_to(tree, name.start..name.end));
    let ty = declared_type(&path, identifier, context.text)?;
    let ty = ty.split('<').next().unwrap_or(ty).trim();
    let simple = ty.strip_prefix("java.util.").or_else(|| ty.strip_prefix("java.lang."));
    let ty = simple.filter(|s| !s.contains('.')).unwrap_or(ty);
    (ty != "var").then_some(ty)
}
