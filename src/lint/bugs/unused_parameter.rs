//! B028: a parameter of a private method or constructor that its body never reads (PMD UnusedFormalParameter with
//! its default `checkAll=false`).
//!
//! As in PMD, parameters named `_` or starting with `ignored` or `unused` are allowed, and methods without a body,
//! `default` methods and the serialization method `readObject(ObjectInputStream) throws InvalidObjectException` are
//! skipped (PMD's exact condition). Assigning a parameter, or incrementing it in a statement of its own, is not
//! reading it. Uses are found by name, without PMD's symbol table: a local variable or lambda parameter of the same
//! name that shadows the parameter counts as a use of it.

use super::shared_b::is_name_reference_in;
use crate::lint::tree::{Context, in_type, path_to};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, MethodDecl, Modifier, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    let text = context.text;
    let Some(body) = method.body.as_deref() else { return };
    if method.compact_ctor
        || !method.mods.has(Modifier::Private)
        || method.mods.has(Modifier::Default)
        || is_serialization_read_object(method, text)
    {
        return;
    }
    let kind = if method.ret.is_some() { "method" } else { "constructor" };
    for param in &method.params {
        let name = param.name.as_str();
        if name == "_" || name == "this" || name.starts_with("ignored") || name.starts_with("unused") {
            continue;
        }
        if !is_read(name, body, text) {
            let message = format!("Avoid unused {kind} parameters such as '{name}'.");
            diagnostics.push(Diagnostic::new(Rule::UnusedParameter, param.name_pos, message));
        }
    }
}

/// `private void readObject(ObjectInputStream in) throws InvalidObjectException`, which PMD skips.
fn is_serialization_read_object(method: &MethodDecl, text: &str) -> bool {
    let is_type = |tree: &Tree, simple: &str, qualified: &str| {
        let name: String = text[tree.start..tree.end].split_whitespace().collect();
        name == simple || name == qualified
    };
    method.name == "readObject"
        && method.receiver.is_none()
        && matches!(method.throws.as_slice(), [t] if is_type(t, "InvalidObjectException", "java.io.InvalidObjectException"))
        && matches!(method.params.as_slice(), [p] if !text[..p.name_pos].trim_end().ends_with("...")
            && p.ty.as_deref().is_some_and(|t| is_type(t, "ObjectInputStream", "java.io.ObjectInputStream")))
}

/// Whether the body reads the name, as PMD's read usages: a name that is not only assigned, nor incremented or
/// compound assigned in a statement of its own.
///
/// Only identifiers spelled like the name can be uses, so its occurrences are found in the text instead of walking
/// the body: an occurrence counts if the body's smallest tree around it is exactly an identifier.
fn is_read(name: &str, body: &Tree, text: &str) -> bool {
    let mut from = body.start;
    while let Some(i) = text[from..body.end].find(name) {
        let position = from + i;
        from = position + name.len();
        let path = path_to(body, position..from);
        let Some(&identifier) = path.last() else { continue };
        if !matches!(identifier.kind, K::Ident(_)) || identifier.start != position || identifier.end != from {
            continue;
        }
        // The ancestor `n` levels up from the identifier, up to the body.
        let up = |n: usize| match path.len().checked_sub(n + 1) {
            Some(i) => Some(path[i]),
            None => (n == path.len()).then_some(body),
        };
        let parent = up(1).unwrap_or(body);
        if in_type(body, &path) || !is_name_reference_in(identifier, parent, up(2)) {
            continue;
        }
        if !is_only_written(identifier, &[up(3), up(2), Some(parent)]) {
            return true;
        }
    }
    false
}

/// Whether the identifier, with its nearest three ancestors (outermost first), is only written: the target of an
/// assignment, or of a compound assignment or increment in a statement of its own.
fn is_only_written(identifier: &Tree, ancestors: &[Option<&Tree>; 3]) -> bool {
    let [great_grandparent, grandparent, parent] = *ancestors;
    let Some(parent) = parent else { return false };
    let is_statement = || match grandparent.map(|g| &g.kind) {
        // Not a `for` initializer, which tree-sitter does not wrap in an expression statement.
        Some(K::ExprStmt(_)) => !great_grandparent.is_some_and(|g| matches!(g.kind, K::For { .. })),
        // The body of a `case ->`, which tree-sitter wraps in an expression statement.
        Some(K::Switch { cases, .. } | K::SwitchExpr { cases, .. }) => {
            cases.iter().any(|c| c.body.as_deref().is_some_and(|b| std::ptr::eq(b, parent)))
        }
        _ => false,
    };
    match &parent.kind {
        K::Assign { op, lhs, .. } if std::ptr::eq(&**lhs, identifier) => op == "=" || is_statement(),
        K::Unary { op, .. } if op == "++" || op == "--" => is_statement(),
        _ => false,
    }
}
