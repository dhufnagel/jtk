//! Helpers on the AST shared by several rules of the group.

use jtk_parser::ast::{K, Tree, VarDecl};

/// Calls `f` on each descendant of `tree` that runs in the same context as it, in source order: not in lambdas or
/// class bodies (local, anonymous and enum constant classes). Annotations are skipped too: their `k = v` arguments
/// are no assignments.
pub(super) fn for_each_in_same_code<'t>(tree: &'t Tree, f: &mut impl FnMut(&'t Tree)) {
    tree.for_each_child(&mut |child| visit_in_same_code(child, f));
}

fn visit_in_same_code<'t>(tree: &'t Tree, f: &mut impl FnMut(&'t Tree)) {
    match &tree.kind {
        K::Lambda { .. } | K::Class(_) | K::Annotation { .. } => {}
        K::NewClass { outer, args, .. } => {
            f(tree);
            outer.iter().for_each(|o| visit_in_same_code(o, f));
            args.iter().for_each(|a| visit_in_same_code(a, f));
        }
        _ => {
            f(tree);
            for_each_in_same_code(tree, f);
        }
    }
}

/// The variable an assignment or `++`/`--` writes to, if it is a plain name.
pub(super) fn modified_name(tree: &Tree) -> Option<&str> {
    let target = match &tree.kind {
        K::Assign { lhs, .. } => lhs,
        K::Unary { op, expr, .. } if matches!(op.as_str(), "++" | "--") => expr,
        _ => return None,
    };
    match &target.kind {
        K::Ident(name) if name != "this" && name != "super" => Some(name),
        _ => None,
    }
}

/// Whether the parameter is a variable arity parameter (`T... values`).
pub(super) fn is_varargs(parameter: &VarDecl, text: &str) -> bool {
    parameter
        .ty
        .as_deref()
        .is_some_and(|ty| matches!(ty.kind, K::ArrayType { .. }) && text[ty.start..ty.end].trim_end().ends_with("..."))
}

/// The members of the class bodies a class declaration holds: its own (unless it is an interface or annotation
/// type, whose bodies tree-sitter does not call class bodies) and those of its enum constants.
pub(super) fn class_bodies(tree: &Tree) -> Vec<&[Tree]> {
    let mut bodies = Vec::new();
    match &tree.kind {
        K::Class(class) => {
            if !matches!(class.kind, jtk_parser::ast::ClassKind::Interface | jtk_parser::ast::ClassKind::Annotation) {
                bodies.push(class.members.as_slice());
            }
            bodies.extend(class.enum_constants.iter().filter_map(|c| c.body.as_deref()));
        }
        K::NewClass { body: Some(body), .. } => bodies.push(body.as_slice()),
        _ => {}
    }
    bodies
}
