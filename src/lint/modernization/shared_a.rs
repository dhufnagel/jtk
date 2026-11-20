//! Helpers of several modernization rules, on the AST.

use crate::lint::tree::strip_parens;
use jtk_parser::ast::{K, Tree};

/// The name that the expression assigns or increments (`x = ...`, `(x) += ...`, `x++`, `--x`) and its identifier, if
/// it is a simple name. `parent` is the expression's parent: an annotation's `key = value` assigns nothing.
pub(super) fn assigned_name<'t>(expression: &'t Tree, parent: &Tree) -> Option<(&'t str, &'t Tree)> {
    let target = match &expression.kind {
        K::Assign { lhs, .. } if !matches!(parent.kind, K::Annotation { .. }) => lhs,
        K::Unary { op, expr, .. } if op == "++" || op == "--" => expr,
        _ => return None,
    };
    let target = strip_parens(target);
    match &target.kind {
        K::Ident(name) => Some((name, target)),
        _ => None,
    }
}

/// Calls `f` on each identifier below `root` that names a variable (not a type, a method or an annotation element),
/// with the trees from `root` down to the identifier's parent; stops when `f` returns `false`.
pub(super) fn for_each_variable_reference<'t>(root: &'t Tree, f: &mut impl FnMut(&'t Tree, &[&'t Tree]) -> bool) {
    let mut path = Vec::new();
    walk_references(root, false, &mut path, f);
}

/// Returns `false` if `f` stopped the walk.
fn walk_references<'t>(
    tree: &'t Tree,
    in_type: bool,
    path: &mut Vec<&'t Tree>,
    f: &mut impl FnMut(&'t Tree, &[&'t Tree]) -> bool,
) -> bool {
    path.push(tree);
    let mut going = true;
    tree.for_each_child(&mut |child| {
        if !going {
            return;
        }
        let child_in_type =
            !matches!(child.kind, K::Annotation { .. }) && (in_type || crate::lint::tree::is_type_child(tree, child));
        if !matches!(child.kind, K::Ident(_)) {
            going = walk_references(child, child_in_type, path, f);
        } else if !child_in_type && !is_method_or_element_name(child, path) {
            going = f(child, path);
        }
    });
    path.pop();
    going
}

/// Whether the identifier names the method of an unqualified call or the element in an annotation's `key = value`;
/// `path` ends with its parent.
fn is_method_or_element_name(identifier: &Tree, path: &[&Tree]) -> bool {
    let is = |t: &Tree| std::ptr::eq(t, identifier);
    match &path[path.len() - 1].kind {
        K::MethodInvocation { select, .. } => is(select),
        K::Assign { lhs, .. } => {
            is(lhs) && path.len() >= 2 && matches!(path[path.len() - 2].kind, K::Annotation { .. })
        }
        _ => false,
    }
}
