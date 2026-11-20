//! B019: double-checked locking on a field that is not `volatile` (PMD DoubleCheckedLocking): other threads can see
//! the reference before the object is fully constructed.
//!
//! `if (x == null) { synchronized (...) { if (x == null) { x = ...; } } }`
//!
//! The outer check may be negated (`if (!(x != null))`). PMD reports the method; jtk reports the outer `if`, which
//! points at the pattern (like Error Prone's DoubleCheckedLocking). PMD also requires a non-primitive return type and a
//! single `return` of the field, which jtk does not: the unsafe publication does not depend on them.

use super::shared_b::for_each_in_same_code;
use crate::lint::tree::{Context, has_modifier, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, LitKind, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::If { then, .. } = &tree.kind else { return };
    let Some(field) = null_tested(tree) else { return };
    let mut inner_checks = false;
    let mut visit = |synchronized: &Tree| {
        if matches!(synchronized.kind, K::Synchronized { .. }) && !inner_checks {
            for_each_in_same_code(synchronized, &mut |inner| {
                inner_checks = inner_checks
                    || matches!(inner.kind, K::If { .. }) && null_tested(inner) == Some(field) && assigns(inner, field);
            });
        }
    };
    for_each_in_same_code(then, &mut visit);
    if inner_checks && is_non_volatile_field(context, field) {
        let message = format!("Double-checked locking on '{field}', which is not volatile");
        diagnostics.push(Diagnostic::new(Rule::DoubleCheckedLocking, tree.start, message));
    }
}

/// The field name `x` in an `if (x == null)` or `if (this.x == null)` condition, or its negation `!(x != null)`.
fn null_tested(statement: &Tree) -> Option<&str> {
    let K::If { cond, .. } = &statement.kind else { return None };
    let mut condition = strip_parens(cond);
    let mut negated = false;
    while let K::Unary { op, expr, .. } = &condition.kind
        && op == "!"
    {
        condition = strip_parens(expr);
        negated = !negated;
    }
    let K::Binary { op, lhs, rhs } = &condition.kind else { return None };
    if op != if negated { "!=" } else { "==" } {
        return None;
    }
    let (left, right) = (strip_parens(lhs), strip_parens(rhs));
    let is_null = |t: &Tree| matches!(t.kind, K::Literal(LitKind::Null));
    let tested = if is_null(right) {
        left
    } else if is_null(left) {
        right
    } else {
        return None;
    };
    field_name(tested)
}

fn field_name(expression: &Tree) -> Option<&str> {
    match &expression.kind {
        K::Ident(name) if name != "this" && name != "super" => Some(name),
        K::MemberSelect { expr, name } if matches!(&expr.kind, K::Ident(t) if t == "this") => Some(name),
        _ => None,
    }
}

fn assigns(statement: &Tree, field: &str) -> bool {
    let mut found = false;
    for_each_in_same_code(statement, &mut |tree| {
        found = found || matches!(&tree.kind, K::Assign { lhs, .. } if field_name(lhs) == Some(field));
    });
    found
}

/// Whether `field` is declared in the class enclosing the checked tree, without `volatile`.
fn is_non_volatile_field(context: &Context, field: &str) -> bool {
    let Some(members) =
        context.ancestors.iter().rev().zip(context.ancestors.iter().rev().skip(1)).find_map(|(child, parent)| {
            match &parent.kind {
                K::Class(class) if matches!(class.kind, ClassKind::Class | ClassKind::Record) => {
                    Some(&class.members[..])
                }
                K::Class(class) if class.kind == ClassKind::Enum => class
                    .enum_constants
                    .iter()
                    .filter_map(|c| c.body.as_deref())
                    .find(|b| b.iter().any(|m| std::ptr::eq(m, *child))),
                K::NewClass { body: Some(body), .. } if body.iter().any(|m| std::ptr::eq(m, *child)) => Some(&body[..]),
                _ => None,
            }
        })
    else {
        return false;
    };
    let declaration = members.iter().find(|m| matches!(&m.kind, K::Vars(vars) if vars.iter().any(|v| v.name == field)));
    declaration.is_some_and(|d| {
        let K::Vars(vars) = &d.kind else { return false };
        !has_modifier(&vars[0].mods, context.text, context.tokens(), "volatile")
    })
}
