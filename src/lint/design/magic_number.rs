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

use super::has_keyword;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, LitKind, Tree};

pub fn check(number: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if !matches!(number.kind, K::Literal(LitKind::Int | LitKind::Long | LitKind::Float | LitKind::Double)) {
        return;
    }
    let ancestors = &context.ancestors;
    if is_in_annotation_element(ancestors) {
        return;
    }
    let source = context.source(number);
    // The sign before the number, outside any parentheses: the parser folds a `-` into a decimal integer.
    let (sign, digits) = match source.strip_prefix('-') {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => (Some(("-", number.start)), rest),
        Some(_) => {
            let token = context.tokens().previous(number.end).unwrap_or(number.start..number.end);
            (Some(("-", number.start)), &context.text[token])
        }
        None => {
            let sign = ancestors.iter().rev().find(|a| !matches!(a.kind, K::Parens(_))).and_then(|a| match &a.kind {
                K::Unary { op, .. } if matches!(op.as_str(), "-" | "+") => Some((op.as_str(), a.start)),
                _ => None,
            });
            (sign, source)
        }
    };
    let is_negative = sign.is_some_and(|(op, _)| op == "-");
    let value = value(digits).map(|v| if is_negative { -v } else { v });
    // Compared like Java's `Arrays.binarySearch`, for which -0.0 is not 0.
    let ignored = &context.options.magic_number.ignore_numbers;
    let is_ignored = value.is_some_and(|v| ignored.iter().any(|n| n.total_cmp(&v).is_eq()));
    if is_ignored || !is_magic(number, ancestors, context) {
        return;
    }
    let (prefix, offset) = sign.unwrap_or(("", number.start));
    let message = format!("'{prefix}{digits}' is a magic number");
    diagnostics.push(Diagnostic::new(Rule::MagicNumber, offset, message));
}

/// Whether one of the trees is an element of an annotation type.
fn is_in_annotation_element(ancestors: &[&Tree]) -> bool {
    ancestors.windows(2).any(|pair| {
        matches!(&pair[0].kind, K::Class(class) if class.kind == ClassKind::Annotation)
            && matches!(pair[1].kind, K::Method(_))
    })
}

/// Whether a number with these ancestors is not part of a constant definition.
fn is_magic(number: &Tree, ancestors: &[&Tree], context: &Context) -> bool {
    let child = |i: usize| ancestors.get(i + 1).copied().unwrap_or(number);
    // The nearest definition of a variable or enum constant.
    let Some(definition) = (0..ancestors.len()).rev().find(|&i| is_definition(ancestors[i], child(i))) else {
        return true;
    };
    if let K::Vars(vars) = &ancestors[definition].kind {
        let is_constant =
            is_in_interface(&ancestors[..definition]) || has_keyword(&vars[0].mods, vars[0].name_pos, "final", context);
        if !is_constant {
            return true;
        }
    }
    // Whether some expression between the definition and the number is not one constants may be built with.
    (definition + 1..ancestors.len()).any(|i| !is_constant_expression(ancestors[i], child(i)))
}

/// Whether the tree defines a variable or enum constant with `child` as its value or argument.
fn is_definition(tree: &Tree, child: &Tree) -> bool {
    match &tree.kind {
        K::Vars(vars) => vars.iter().any(|v| v.init.as_deref().is_some_and(|init| std::ptr::eq(init, child))),
        K::Class(class) => class.enum_constants.iter().any(|c| c.args.iter().any(|a| std::ptr::eq(a, child))),
        _ => false,
    }
}

/// Whether the nearest enclosing type of a declaration is an interface or annotation type, as Checkstyle sees it:
/// it stops at any `new` expression.
fn is_in_interface(ancestors: &[&Tree]) -> bool {
    ancestors
        .iter()
        .rev()
        .find(|a| matches!(a.kind, K::Class(_) | K::NewClass { .. } | K::NewArray { .. }))
        .is_some_and(|a| matches!(&a.kind, K::Class(class) if matches!(class.kind, ClassKind::Interface | ClassKind::Annotation)))
}

/// Whether a constant may contain an expression of the kind of `tree` around its `child` (Checkstyle's
/// `constantWaiverParentToken`).
fn is_constant_expression(tree: &Tree, child: &Tree) -> bool {
    let is_child = |t: &Tree| std::ptr::eq(t, child);
    match &tree.kind {
        K::Parens(_) | K::Cast { .. } | K::Conditional { .. } => true,
        K::NewClass { body, .. } => !body.iter().flatten().any(is_child),
        K::NewArray { init, .. } => init.iter().flatten().any(is_child),
        K::Binary { op, .. } => !matches!(op.as_str(), "&&" | "||"),
        K::Unary { op, .. } => op != "!",
        K::Assign { op, .. } => op == "=",
        K::MethodInvocation { args, .. } => args.iter().any(is_child),
        _ => false,
    }
}

/// The value of a numeric literal, as Checkstyle parses it.
fn value(literal: &str) -> Option<f64> {
    let literal = literal.replace('_', "");
    let lower = literal.to_ascii_lowercase();
    let is_hex = lower.starts_with("0x");
    let is_float = if is_hex { lower.contains('p') } else { lower.contains(['.', 'e']) || lower.ends_with(['f', 'd']) };
    if is_float {
        // Hexadecimal floating point literals are not parsed.
        return if is_hex { None } else { lower.trim_end_matches(['f', 'd']).parse().ok() };
    }
    let is_long = lower.ends_with('l');
    let lower = lower.trim_end_matches('l');
    let (digits, radix) = if let Some(hex) = lower.strip_prefix("0x") {
        (hex, 16)
    } else if let Some(binary) = lower.strip_prefix("0b") {
        (binary, 2)
    } else if lower.len() > 1 && lower.starts_with('0') {
        (&lower[1..], 8)
    } else {
        (lower, 10)
    };
    // Like Java's `parseUnsignedInt` and `parseUnsignedLong`: `0xffffffff` is -1.
    let value = u64::from_str_radix(digits, radix).ok()?;
    Some(if is_long { value as i64 as f64 } else { value as u32 as i32 as f64 })
}
