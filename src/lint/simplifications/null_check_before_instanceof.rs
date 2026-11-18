//! S020: `x != null` combined with `&&` with `x instanceof T`, which is false for `null` anyway (Checkstyle
//! UnnecessaryNullCheckWithInstanceOf, PMD SimplifyConditional). Following Checkstyle, the null check may be anywhere
//! in the `&&` chain, but not after `x` was dereferenced (`x != null && x.isValid() && x instanceof T`), and not in an
//! `||` alternative to the `instanceof`. Like Checkstyle, only conditions whose outermost `&&`/`||` operation is an
//! `&&` are checked (not `x != null && x instanceof T || y`), and a null check before the `instanceof` is reported
//! even if the `instanceof` is in an `||` inside the chain (`x != null && (x instanceof T || y)`). A null check after
//! the `instanceof` is reported only if the `instanceof` is itself in the chain.
//!
//! The fix removes the null check if it is next to the `instanceof` in the chain, so that nothing else is evaluated
//! with a `null` `x` that was not before.

use crate::ast::{K, LitKind, Tree};
use crate::lint::tree::{Context, is_type_child, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(instanceof: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(name) = tested_variable(instanceof) else { return };
    let mut top = instanceof;
    for parent in context.ancestors.iter().rev() {
        if is_operation(parent, "&&") || is_operation(parent, "||") {
            top = parent;
        } else if !matches!(parent.kind, K::Parens(_)) {
            break;
        }
    }
    if !is_operation(top, "&&") {
        return;
    }
    // Several `instanceof`s of the variable in one condition share its null check: report it once, for the first.
    let Some((first, null_check)) = redundant_null_check(top, name) else { return };
    if !std::ptr::eq(first, instanceof) {
        return;
    }
    let mut diagnostic =
        Diagnostic::new(Rule::NullCheckBeforeInstanceof, null_check.start, "Unnecessary null check before instanceof");
    if let Some(edit) = removal(top, null_check, instanceof, context) {
        diagnostic = diagnostic.with_fix(vec![edit]);
    }
    diagnostics.push(diagnostic);
}

/// An `instanceof` of the variable in a condition, with whether only `&&` operations (and parentheses) lie between
/// it and the condition, and the start of the first dereference of the variable.
struct Found<'t> {
    instanceofs: Vec<(&'t Tree, bool)>,
    first_dereference: usize,
}

/// The first `instanceof` of the variable in the `&&` condition `top` that makes a null check of it in the `&&`
/// chain redundant, with that null check.
///
/// The `instanceof` must come before any dereference of the variable in the condition (which the null check guards
/// against). A null check before the `instanceof` is reported also when the `instanceof` is in an `||` alternative,
/// as in Checkstyle; a null check after it only when the `instanceof` is itself in the `&&` chain, so that it is
/// true whenever the null check is evaluated.
fn redundant_null_check<'t>(top: &'t Tree, name: &str) -> Option<(&'t Tree, &'t Tree)> {
    let mut found = Found { instanceofs: Vec::new(), first_dereference: usize::MAX };
    search(top, name, Some(true), false, &mut found);
    let null_checks: Vec<&Tree> =
        flatten(top).into_iter().map(|(leaf, _)| strip_parens(leaf)).filter(|l| is_null_check(l, name)).collect();
    let first_dereference = found.first_dereference;
    found.instanceofs.into_iter().filter(|(i, _)| i.start < first_dereference).find_map(|(instanceof, in_chain)| {
        let null_check = null_checks.iter().find(|n| n.start < instanceof.start || in_chain)?;
        Some((instanceof, *null_check))
    })
}

/// Collects the `instanceof`s of the variable below `tree` whose outermost `&&`/`||` operation is `top`, and the
/// dereferences of the variable. `chain` tells, while only `&&`/`||` operations and parentheses lie between `tree`
/// and `top`, whether they are all `&&` operations.
fn search<'t>(tree: &'t Tree, name: &str, chain: Option<bool>, in_type: bool, found: &mut Found<'t>) {
    match &tree.kind {
        K::MemberSelect { expr, name: member } if !in_type && member != "class" => {
            if matches!(&expr.kind, K::Ident(n) if n == name) {
                found.first_dereference = found.first_dereference.min(tree.start);
            }
        }
        K::InstanceOf { .. } if tested_variable(tree) == Some(name) => {
            if let Some(in_chain) = chain {
                found.instanceofs.push((tree, in_chain));
            }
        }
        _ => {}
    }
    let chain = match &tree.kind {
        K::Parens(_) => chain,
        _ if is_operation(tree, "&&") => chain,
        _ if is_operation(tree, "||") => chain.map(|_| false),
        _ => None,
    };
    tree.for_each_child(&mut |child| {
        let child_in_type = !matches!(child.kind, K::Annotation { .. }) && (in_type || is_type_child(tree, child));
        search(child, name, chain, child_in_type, found);
    });
}

/// The variable name an `instanceof` tests, if its operand is a plain name.
fn tested_variable(instanceof: &Tree) -> Option<&str> {
    let K::InstanceOf { expr, .. } = &instanceof.kind else { return None };
    match &strip_parens(expr).kind {
        K::Ident(name) if name != "this" && name != "super" => Some(name),
        _ => None,
    }
}

/// Whether the tree is `name != null` or `null != name`.
fn is_null_check(tree: &Tree, name: &str) -> bool {
    let K::Binary { op, lhs, rhs } = &tree.kind else { return false };
    let is_name = |t: &Tree| matches!(&t.kind, K::Ident(n) if n == name);
    let is_null = |t: &Tree| matches!(t.kind, K::Literal(LitKind::Null));
    op == "!=" && (is_name(lhs) && is_null(rhs) || is_null(lhs) && is_name(rhs))
}

fn is_operation(tree: &Tree, operator: &str) -> bool {
    matches!(&tree.kind, K::Binary { op, .. } if op == operator)
}

/// The edit that removes the null check from its `&&` operation, if the `instanceof` is next to it in the `&&`
/// chain.
fn removal(top: &Tree, null_check: &Tree, instanceof: &Tree, context: &Context) -> Option<Edit> {
    let leaves = flatten(top);
    let index = leaves.iter().position(|(leaf, _)| std::ptr::eq(strip_parens(leaf), null_check))?;
    let is_neighbour = |i: Option<usize>| {
        i.and_then(|i| leaves.get(i)).is_some_and(|(l, _)| std::ptr::eq(strip_parens(l), instanceof))
    };
    if !is_neighbour(index.checked_sub(1)) && !is_neighbour(Some(index + 1)) {
        return None;
    }
    let (operand, and) = leaves[index];
    let K::Binary { lhs, rhs, .. } = &and.kind else { return None };
    let range = if std::ptr::eq(&**lhs, operand) { lhs.start..rhs.start } else { lhs.end..rhs.end };
    if context.tokens().has_comment(range.clone()) {
        return None;
    }
    Some(Edit::new(range, ""))
}

/// The operands of a chain of `&&` operations (through parentheses) in order, each with the operation it is an
/// operand of.
fn flatten(and: &Tree) -> Vec<(&Tree, &Tree)> {
    let mut leaves = Vec::new();
    // Operands still to visit with their operation, the next one last.
    let mut pending: Vec<(&Tree, &Tree)> = Vec::new();
    if let K::Binary { lhs, rhs, .. } = &and.kind {
        pending.extend([(&**rhs, and), (&**lhs, and)]);
    }
    while let Some((operand, operation)) = pending.pop() {
        let inner = strip_parens(operand);
        match &inner.kind {
            K::Binary { op, lhs, rhs } if op == "&&" => pending.extend([(&**rhs, inner), (&**lhs, inner)]),
            _ => leaves.push((operand, operation)),
        }
    }
    leaves
}
