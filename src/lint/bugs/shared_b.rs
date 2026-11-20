//! Helpers on the AST shared by several rules of the group.

use jtk_parser::ast::{ClassKind, K, LitKind, Tree, VarDecl};

/// Calls `f` on each descendant of `tree` in source order, without entering lambdas and class bodies (which are
/// visited themselves): code there runs in a different context. Like tree-sitter's `class_body`, the bodies of
/// interfaces, enums (but not of their constants) and annotation types are entered.
pub(super) fn for_each_in_same_code<'t>(tree: &'t Tree, f: &mut impl FnMut(&'t Tree)) {
    tree.for_each_child(&mut |child| {
        f(child);
        if !matches!(child.kind, K::Lambda { .. }) && !in_class_body(tree, child) {
            for_each_in_same_code(child, f);
        }
    });
}

/// Whether `child` of `parent` is a member of a class body: of a class or record, an anonymous class or an enum
/// constant.
fn in_class_body(parent: &Tree, child: &Tree) -> bool {
    match &parent.kind {
        K::Class(class) => match class.kind {
            ClassKind::Class | ClassKind::Record => class.members.first().is_some_and(|m| m.start <= child.start),
            ClassKind::Enum => class.enum_constants.iter().any(|c| in_members(c.body.as_deref(), child)),
            ClassKind::Interface | ClassKind::Annotation => false,
        },
        K::NewClass { body, .. } => in_members(body.as_deref(), child),
        _ => false,
    }
}

/// Whether `child` is one of the members (or in one).
fn in_members(members: Option<&[Tree]>, child: &Tree) -> bool {
    members.is_some_and(|m| {
        m.first().is_some_and(|f| f.start <= child.start) && m.last().is_some_and(|l| child.end <= l.end)
    })
}

/// The declared type (its source) of the local variable, parameter or field `name` visible at the end of `path` (the
/// trees from the compilation unit down). `None` if no declaration is found in the enclosing code, if the name is
/// declared with extra array dimensions (`int a[]`) or as an implicitly typed lambda parameter, and if it may be a
/// field inherited by an enclosing class (which has a supertype and does not declare the name itself). `var` is the
/// type of a variable declared with it.
pub(super) fn declared_type<'t>(path: &[&'t Tree], name: &str, text: &'t str) -> Option<&'t str> {
    for pair in path.windows(2).rev() {
        let (parent, child) = (pair[0], pair[1]);
        if let Some(found) = declaration_in(parent, child, name, text) {
            return found;
        }
        let may_inherit = match &parent.kind {
            K::Class(class) => class.extends.is_some() || !class.implements.is_empty(),
            _ => false,
        };
        if may_inherit {
            return None;
        }
    }
    None
}

/// The declaration of `name` in `parent` that is visible in its child `child`: `Some` with the type if found, or with
/// `None` if the search stops there without a type.
fn declaration_in<'t>(parent: &'t Tree, child: &Tree, name: &str, text: &'t str) -> Option<Option<&'t str>> {
    let found = |ty: Option<&'t str>| ty.map(Some);
    match &parent.kind {
        K::Block { stmts, .. } => {
            found(stmts.iter().filter(|s| s.end <= child.start).filter_map(|s| vars_type(s, name, text)).next_back())
        }
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => found(
            cases
                .iter()
                .flat_map(|c| &c.stmts)
                .filter(|s| s.end <= child.start)
                .filter_map(|s| vars_type(s, name, text))
                .next_back(),
        ),
        K::Class(class) => {
            let in_class_members = class.members.first().is_some_and(|m| m.start <= child.start);
            let constant_body = class
                .enum_constants
                .iter()
                .filter_map(|c| c.body.as_deref())
                .find(|body| in_members(Some(body), child));
            let field = if in_class_members && class.kind != ClassKind::Annotation {
                class.members.iter().find_map(|m| vars_type(m, name, text))
            } else {
                constant_body.and_then(|body| body.iter().find_map(|m| vars_type(m, name, text)))
            };
            let component = || class.record_components.iter().find_map(|c| parameter_type(c, name, text));
            found(field.or_else(component))
        }
        K::NewClass { body: Some(body), .. } if in_members(Some(body), child) => {
            Some(body.iter().find_map(|m| vars_type(m, name, text)))
        }
        K::For { init, .. } => found(init.first().and_then(|i| vars_type(i, name, text))),
        K::ForEach { var, .. } if var.name == name => found(type_without_dimensions(var, text)),
        K::Try { resources, catches, .. } => {
            let catch = catches.iter().find(|c| c.start <= child.start && child.end <= c.block.end);
            if let Some(catch) = catch.filter(|c| c.param.name == name) {
                return Some(catch.param.ty.as_deref().map(|t| &text[t.start..t.end]));
            }
            found(resources.iter().find_map(|r| match &r.kind {
                K::Var(var) if var.name == name => type_without_dimensions(var, text),
                _ => None,
            }))
        }
        K::Method(method) => found(method.params.iter().find_map(|p| parameter_type(p, name, text))),
        K::Lambda { params, .. } => match params.iter().find(|p| p.name == name) {
            Some(VarDecl { ty: None, .. }) => Some(None),
            Some(param) => found(parameter_type(param, name, text)),
            None => None,
        },
        _ => None,
    }
}

/// The type of a local variable or field declaration that declares `name` without extra array dimensions.
fn vars_type<'t>(declaration: &Tree, name: &str, text: &'t str) -> Option<&'t str> {
    let K::Vars(vars) = &declaration.kind else { return None };
    type_without_dimensions(vars.iter().find(|v| v.name == name)?, text)
}

/// The type of the parameter `name`, if it is not a variable arity parameter.
fn parameter_type<'t>(parameter: &VarDecl, name: &str, text: &'t str) -> Option<&'t str> {
    let is_variable_arity = text[..parameter.name_pos].trim_end().ends_with("...");
    (parameter.name == name && !is_variable_arity).then(|| type_without_dimensions(parameter, text)).flatten()
}

/// The type of the variable unless it has dimensions after its name; `var` if it has no type.
fn type_without_dimensions<'t>(var: &VarDecl, text: &'t str) -> Option<&'t str> {
    match var.ty.as_deref() {
        None => Some("var"),
        Some(ty) if ty.end > var.name_pos => None,
        Some(ty) => Some(&text[ty.start..ty.end]),
    }
}

/// Whether the identifier `identifier` (with its parent and grandparent) names a variable, unless it is in a type: not
/// a method or an annotation element.
pub(super) fn is_name_reference_in(identifier: &Tree, parent: &Tree, grandparent: Option<&Tree>) -> bool {
    match &parent.kind {
        K::MethodInvocation { select, .. } => !std::ptr::eq(&**select, identifier),
        K::Assign { lhs, .. } if std::ptr::eq(&**lhs, identifier) => {
            !grandparent.is_some_and(|g| matches!(g.kind, K::Annotation { .. }))
        }
        _ => true,
    }
}

/// Whether evaluating the expression has no side effects, like tree-sitter's names, field accesses on names, and
/// literals (decimal and hexadecimal integers, decimal floating point numbers, not negative ones).
pub(super) fn is_side_effect_free(expression: &Tree, text: &str) -> bool {
    match &expression.kind {
        K::Ident(name) => name != "super",
        K::Literal(kind) => is_simple_literal(*kind, &text[expression.start..expression.end]),
        K::MemberSelect { expr, name } => name != "class" && is_side_effect_free(expr, text),
        K::Parens(inner) => is_side_effect_free(inner, text),
        _ => false,
    }
}

/// Whether the literal is one of tree-sitter's side-effect-free literals: not a negative number (a unary expression
/// in tree-sitter), an octal or binary integer or a hexadecimal floating point number.
pub(super) fn is_simple_literal(kind: LitKind, literal: &str) -> bool {
    match kind {
        _ if literal.starts_with('-') => false,
        LitKind::Null | LitKind::Boolean | LitKind::String | LitKind::Char => true,
        LitKind::Int | LitKind::Long => {
            !literal.starts_with('0') || literal.len() == 1 || literal[1..].starts_with(['l', 'L', 'x', 'X'])
        }
        LitKind::Float | LitKind::Double => !literal.starts_with("0x") && !literal.starts_with("0X"),
    }
}
