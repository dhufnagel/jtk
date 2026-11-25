//! Helpers of the rules S028 to S137 that need what little the file tells about types: the declared types of local
//! names and what a type name written in the file refers to.

use crate::lint::tree::{Context, for_each_descendant};
use jtk_parser::ast::{ClassDecl, ClassKind, K, Tree, VarDecl};

/// The declared type of the local variable, parameter or field `name` visible at `tree`, below `ancestors`
/// (outermost first). `None` if no declaration is found in the enclosing code, if the name is declared with `var`,
/// extra array dimensions (`int a[]`) or as an implicitly typed lambda parameter, if a pattern variable of that name
/// may be in scope, and if it may be a field inherited by an enclosing class (which has a supertype and does not
/// declare the name itself).
pub(super) fn declared_type<'t>(
    tree: &'t Tree,
    ancestors: &[&'t Tree],
    name: &str,
    context: &Context,
) -> Option<&'t Tree> {
    declaration(tree, ancestors, name, context).and_then(|(ty, _)| ty)
}

/// Whether `name` at `tree` is a local variable or parameter (of a method, constructor, lambda or catch), not a
/// field.
pub(super) fn is_local_variable<'t>(tree: &'t Tree, ancestors: &[&'t Tree], name: &str, context: &Context) -> bool {
    declaration(tree, ancestors, name, context).is_some_and(|(_, local)| local)
}

/// The declaration of `name` visible at `tree`: its type (`None` if not usable) and whether it is local.
fn declaration<'t>(
    tree: &'t Tree,
    ancestors: &[&'t Tree],
    name: &str,
    context: &Context,
) -> Option<(Option<&'t Tree>, bool)> {
    let mut child = tree;
    for &parent in ancestors.iter().rev() {
        let local = !matches!(parent.kind, K::Class(_) | K::NewClass { .. });
        if let Some(found) = declaration_in(parent, child, name, context) {
            return Some((found, local));
        }
        child = parent;
    }
    None
}

/// The type of the declaration of `name` that `parent` makes visible to its child `child`: `Some(None)` if the
/// declaration has no usable type, or if the name may be inherited or be a pattern variable, so that the search stops
/// there.
fn declaration_in<'t>(parent: &'t Tree, child: &Tree, name: &str, context: &Context) -> Option<Option<&'t Tree>> {
    let before = |s: &&Tree| s.end <= child.start;
    let is_child = |t: &Tree| std::ptr::eq(t, child);
    if binds_in_condition(parent, name) {
        return Some(None);
    }
    match &parent.kind {
        K::Block { stmts, .. } => statements_before(stmts.iter().filter(before), name),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            if cases.iter().flat_map(|c| &c.labels).any(|l| has_binding(l, name)) {
                return Some(None);
            }
            statements_before(cases.iter().flat_map(|c| &c.stmts).filter(before), name)
        }
        K::For { init, .. } => init.iter().find_map(|s| declared_in(s, name)),
        K::ForEach { var, .. } if var.name == name => Some(unless_dimensions(var)),
        K::Try { resources, catches, .. } => {
            let resource = |r: &'t Tree| match &r.kind {
                K::Var(var) if var.name == name => Some(unless_dimensions(var)),
                _ => None,
            };
            let catch = catches.iter().find(|c| c.start <= child.start && child.end <= c.block.end);
            if let Some(catch) = catch.filter(|c| c.param.name == name) {
                return Some(if catch.union.is_empty() { catch.param.ty.as_deref() } else { None });
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
                Some(_) => return Some(None),
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

/// The declaration of `name` among the statements before a tree, the last one first; `Some(None)` if a pattern
/// variable of that name may be in scope after one of them (`if (!(o instanceof String name)) return;`).
fn statements_before<'t>(
    statements: impl DoubleEndedIterator<Item = &'t Tree>,
    name: &str,
) -> Option<Option<&'t Tree>> {
    for statement in statements.rev() {
        if let Some(found) = declared_in(statement, name) {
            return Some(found);
        }
        if binds_in_condition(statement, name) {
            return Some(None);
        }
    }
    None
}

/// Whether the condition of the statement or expression declares a pattern variable named `name`.
fn binds_in_condition(tree: &Tree, name: &str) -> bool {
    match &tree.kind {
        K::If { cond, .. } | K::While { cond, .. } | K::DoWhile { cond, .. } | K::Conditional { cond, .. } => {
            has_binding(cond, name)
        }
        K::For { cond: Some(cond), .. } => has_binding(cond, name),
        K::Binary { op, lhs, .. } if op == "&&" || op == "||" => has_binding(lhs, name),
        _ => false,
    }
}

/// Whether a pattern below the tree declares a variable named `name`.
fn has_binding(tree: &Tree, name: &str) -> bool {
    let mut found = false;
    for_each_descendant(tree, &mut |t, _| {
        found |= matches!(&t.kind, K::BindingPattern { var } if var.name == name);
        !found
    });
    found
}

/// The type of a local variable or field declaration that declares `name`.
fn declared_in<'t>(declaration: &'t Tree, name: &str) -> Option<Option<&'t Tree>> {
    let K::Vars(vars) = &declaration.kind else { return None };
    Some(unless_dimensions(vars.iter().find(|v| v.name == name)?))
}

/// The type of a variable: `None` for `var` and with extra array dimensions after the name.
fn unless_dimensions(var: &VarDecl) -> Option<&Tree> {
    var.ty.as_deref().filter(|ty| ty.end <= var.name_pos)
}

/// The type of a parameter named `name`; `None` inside if it is a variable arity one.
fn parameter_type<'t>(param: &'t VarDecl, name: &str, context: &Context) -> Option<Option<&'t Tree>> {
    if param.name != name {
        return None;
    }
    let is_variable_arity = context.tokens().previous(param.name_pos).is_some_and(|t| &context.text[t] == "...");
    Some(if is_variable_arity { None } else { unless_dimensions(param) })
}

/// The name of a class type without its type arguments (`List` of `List<String>`), as written; `None` for other
/// types.
pub(super) fn class_name<'t>(ty: &'t Tree, context: &Context<'t>) -> Option<String> {
    match &ty.kind {
        K::Ident(_) | K::MemberSelect { .. } => Some(without_whitespace(context.source(ty))),
        K::Parameterized { base, .. } => class_name(base, context),
        _ => None,
    }
}

pub(super) fn without_whitespace(code: &str) -> String {
    code.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Whether the type name `written` at the end of `ancestors` refers to the class `qualified` (such as
/// `java.util.List`): written fully qualified, or simply and imported (by a single-type import, or on demand from
/// its package without a single-type import of the same name), or of `java.lang` and not imported from elsewhere.
/// A type of the same name declared in the file or a type parameter of the enclosing code hides it.
pub(super) fn refers_to(written: &str, qualified: &str, ancestors: &[&Tree], context: &Context) -> bool {
    if written == qualified {
        return true;
    }
    let Some((package, simple)) = qualified.rsplit_once('.') else { return false };
    if written != simple || declares_type(ancestors, simple) {
        return false;
    }
    let Some(K::CompilationUnit { imports, .. }) = ancestors.first().map(|u| &u.kind) else { return false };
    let mut on_demand = false;
    for import in imports {
        let K::Import { is_static: false, name } = &import.kind else { continue };
        let name = without_whitespace(context.source(name));
        match name.rsplit_once('.') {
            Some((_, last)) if last == simple => return name == qualified,
            Some((imported, "*")) => on_demand |= imported == package,
            _ => {}
        }
    }
    on_demand || package == "java.lang"
}

/// Whether a type named `name` is declared in the file (a top-level or member class) or in the code enclosing the
/// end of `ancestors` (a local class or a type parameter).
pub(super) fn declares_type(ancestors: &[&Tree], name: &str) -> bool {
    let unit = ancestors.first().copied();
    let member = unit.is_some_and(|u| !member_classes(u, name).is_empty());
    member || declares_locally(ancestors, name)
}

/// Whether the code enclosing the end of `ancestors` declares a local class or type parameter named `name`.
pub(super) fn declares_locally(ancestors: &[&Tree], name: &str) -> bool {
    ancestors.iter().any(|a| declares_in(a, name))
}

/// Whether the file has a single-type import of a type named `name`.
pub(super) fn imports(ancestors: &[&Tree], name: &str) -> bool {
    let Some(K::CompilationUnit { imports, .. }) = ancestors.first().map(|u| &u.kind) else { return false };
    imports.iter().any(|import| match &import.kind {
        K::Import { is_static: false, name: imported } => {
            matches!(&imported.kind, K::MemberSelect { name: n, .. } if n == name)
        }
        _ => false,
    })
}

/// The top-level and member classes named `name` in the compilation unit, not those of method bodies.
pub(super) fn member_classes<'t>(unit: &'t Tree, name: &str) -> Vec<&'t ClassDecl> {
    fn collect<'t>(trees: &'t [Tree], name: &str, found: &mut Vec<&'t ClassDecl>) {
        for tree in trees {
            if let K::Class(class) = &tree.kind {
                if class.name == name {
                    found.push(class);
                }
                collect(&class.members, name, found);
            }
        }
    }
    let mut found = Vec::new();
    if let K::CompilationUnit { types, .. } = &unit.kind {
        collect(types, name, &mut found);
    }
    found
}

/// Whether the tree declares a type parameter or a local class named `name` for the code inside it.
fn declares_in(tree: &Tree, name: &str) -> bool {
    let is_parameter = |p: &Tree| matches!(&p.kind, K::TypeParameter { name: n, .. } if n == name);
    match &tree.kind {
        K::Class(class) => class.type_params.iter().any(is_parameter),
        K::Method(method) => method.type_params.iter().any(is_parameter),
        K::Block { stmts, .. } => stmts.iter().any(|s| matches!(&s.kind, K::Class(c) if c.name == name)),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            cases.iter().flat_map(|c| &c.stmts).any(|s| matches!(&s.kind, K::Class(c) if c.name == name))
        }
        _ => false,
    }
}

/// Whether the variable `name` is assigned, incremented or decremented in the tree.
pub(super) fn is_assigned_in(tree: &Tree, name: &str) -> bool {
    let is_name = |t: &Tree| matches!(&crate::lint::tree::strip_parens(t).kind, K::Ident(n) if n == name);
    let mut found = false;
    for_each_descendant(tree, &mut |t, _| {
        found |= match &t.kind {
            K::Assign { lhs, .. } => is_name(lhs),
            K::Unary { op, expr, .. } => (op == "++" || op == "--") && is_name(expr),
            _ => false,
        };
        !found
    });
    found
}
