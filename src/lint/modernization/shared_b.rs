//! Helpers on the AST shared by several rules of the group.

use crate::lint::tree::{Context, is_type_child};
use jtk_parser::ast::{ClassKind, K, Tree, VarDecl};

/// Whether `tree`, a child of `parent` outside types, names a variable: an identifier that is not a method name,
/// `this` or `super`.
pub(super) fn is_variable_name(tree: &Tree, parent: &Tree) -> bool {
    let K::Ident(name) = &tree.kind else { return false };
    let is_method_name = matches!(&parent.kind, K::MethodInvocation { select, .. } if std::ptr::eq(&**select, tree));
    !is_method_name && name != "this" && name != "super"
}

/// Calls `f` on each name of a variable below `tree` (see [`is_variable_name`]) in source order, with the trees from
/// `tree` down to its parent. Types, the names of annotation elements and names stored as strings (declarations,
/// labels, members of a `MemberSelect`) are left out.
pub(super) fn for_each_variable_name<'t>(tree: &'t Tree, f: &mut impl FnMut(&'t Tree, &[&'t Tree])) {
    fn walk<'t>(tree: &'t Tree, ancestors: &mut Vec<&'t Tree>, f: &mut impl FnMut(&'t Tree, &[&'t Tree])) {
        if let Some(parent) = ancestors.last()
            && is_variable_name(tree, parent)
        {
            f(tree, ancestors);
            return;
        }
        ancestors.push(tree);
        tree.for_each_child(&mut |child| {
            let element_name = matches!(tree.kind, K::Annotation { .. }) && matches!(child.kind, K::Assign { .. });
            match &child.kind {
                K::Assign { rhs, .. } if element_name => {
                    ancestors.push(child);
                    walk(rhs, ancestors, f);
                    ancestors.pop();
                }
                _ if is_type_child(tree, child) => {}
                _ => walk(child, ancestors, f),
            }
        });
        ancestors.pop();
    }
    walk(tree, &mut Vec::new(), f);
}

/// The declared type of the local variable, parameter or field `name` visible at `tree`, below `ancestors`
/// (outermost first). `None` if no declaration is found in the enclosing code, if the name is declared with `var`,
/// extra array dimensions (`int a[]`) or as an implicitly typed lambda parameter, and if it may be a field inherited
/// by an enclosing class (which has a supertype and does not declare the name itself).
pub(super) fn declared_type<'t>(
    tree: &'t Tree,
    ancestors: &[&'t Tree],
    name: &str,
    context: &Context,
) -> Option<&'t Tree> {
    let mut child = tree;
    for &parent in ancestors.iter().rev() {
        if let Some(found) = declaration_in(parent, child, name, context) {
            return found;
        }
        child = parent;
    }
    None
}

/// The type of the declaration of `name` that `parent` makes visible to its child `child`: `Some(None)` if the
/// declaration has no usable type, or if the name may be inherited, so that the search stops there.
fn declaration_in<'t>(parent: &'t Tree, child: &Tree, name: &str, context: &Context) -> Option<Option<&'t Tree>> {
    let before = |s: &&Tree| s.end <= child.start;
    let is_child = |t: &Tree| std::ptr::eq(t, child);
    match &parent.kind {
        K::Block { stmts, .. } => stmts.iter().filter(before).filter_map(|s| declared_in(s, name)).next_back(),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            cases.iter().flat_map(|c| &c.stmts).filter(before).filter_map(|s| declared_in(s, name)).next_back()
        }
        K::For { init, .. } => init.iter().find_map(|s| declared_in(s, name)),
        K::ForEach { var, .. } if var.name == name => unless_dimensions(var),
        K::Try { resources, catches, .. } => {
            let resource = |r: &'t Tree| match &r.kind {
                K::Var(var) if var.name == name => unless_dimensions(var),
                _ => None,
            };
            let catch = catches.iter().find(|c| c.start <= child.start && child.end <= c.block.end);
            if let Some(catch) = catch.filter(|c| c.param.name == name) {
                return Some(catch.param.ty.as_deref());
            }
            let earlier =
                if resources.iter().any(is_child) { resources.iter().filter(before).find_map(resource) } else { None };
            earlier.or_else(|| resources.iter().find_map(resource))
        }
        K::Method(method) => method.params.iter().find_map(|p| parameter_type(p, name, context)),
        K::Lambda { params, .. } => {
            let param = params.iter().find(|p| p.name == name)?;
            if param.ty.is_none() { Some(None) } else { parameter_type(param, name, context) }
        }
        K::Class(class) => {
            let constant_body =
                class.enum_constants.iter().find_map(|c| c.body.as_deref().filter(|b| b.iter().any(is_child)));
            let body: &[Tree] = match constant_body {
                Some(body) => body,
                None if class.kind != ClassKind::Annotation && class.members.iter().any(is_child) => &class.members,
                None => &[],
            };
            let components = if class.kind == ClassKind::Record { &class.record_components[..] } else { &[] };
            let found = body
                .iter()
                .find_map(|m| declared_in(m, name))
                .or_else(|| components.iter().find_map(|p| parameter_type(p, name, context)));
            let may_inherit = match class.kind {
                ClassKind::Class | ClassKind::Enum | ClassKind::Record => {
                    class.extends.is_some() || !class.implements.is_empty()
                }
                ClassKind::Interface => !class.implements.is_empty(),
                ClassKind::Annotation => false,
            };
            found.or(may_inherit.then_some(None))
        }
        // An anonymous class may inherit the name.
        K::NewClass { body: Some(body), .. } if body.iter().any(is_child) => {
            Some(body.iter().find_map(|m| declared_in(m, name)).flatten())
        }
        _ => None,
    }
}

/// The type of a local variable or field declaration that declares `name`.
fn declared_in<'t>(declaration: &'t Tree, name: &str) -> Option<Option<&'t Tree>> {
    let K::Vars(vars) = &declaration.kind else { return None };
    unless_dimensions(vars.iter().find(|v| v.name == name)?)
}

/// The type of a variable (`None` for `var`), unless it has extra array dimensions after the name.
fn unless_dimensions(var: &VarDecl) -> Option<Option<&Tree>> {
    match var.ty.as_deref() {
        Some(ty) if ty.end > var.name_pos => None,
        ty => Some(ty),
    }
}

/// The type of a parameter (not a variable arity one) named `name`.
fn parameter_type<'t>(param: &'t VarDecl, name: &str, context: &Context) -> Option<Option<&'t Tree>> {
    let is_variable_arity = context.tokens().previous(param.name_pos).is_some_and(|t| &context.text[t] == "...");
    if param.name != name || is_variable_arity {
        return None;
    }
    unless_dimensions(param)
}
