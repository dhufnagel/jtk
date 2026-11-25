//! Helpers shared by the rules of the group: the variables declared in the file, the types known locally and the
//! resolution of type names through the file's imports.

use crate::lint::tree::{Context, for_each_descendant, path_to, strip_parens};
use jtk_parser::ast::{ClassDecl, ClassKind, K, Modifier, Tree, VarDecl};

/// A variable declared in the file: a local variable, parameter, field or record component.
pub(super) struct Variable<'t> {
    pub decl: &'t VarDecl,
    /// The kind of class declaring the variable if it is a field or record component (`Class` for anonymous classes).
    pub owner: Option<ClassKind>,
    /// The declared type, `None` if it is not known: with extra array dimensions (`int a[]`), a variable arity
    /// parameter, a multi-catch parameter or `var` without a `new X()` initializer.
    pub ty: Option<&'t Tree>,
}

impl Variable<'_> {
    pub fn is_field(&self) -> bool {
        self.owner.is_some()
    }

    /// Whether the variable is a static field (fields of interfaces are implicitly static).
    pub fn is_static(&self) -> bool {
        self.owner.is_some_and(|owner| self.decl.mods.has(Modifier::Static) || is_interface(owner))
    }

    /// Whether the variable is final (fields of interfaces and record components are implicitly final).
    pub fn is_final(&self) -> bool {
        self.decl.mods.has(Modifier::Final) || self.decl.is_record_component || self.owner.is_some_and(is_interface)
    }
}

fn is_interface(kind: ClassKind) -> bool {
    matches!(kind, ClassKind::Interface | ClassKind::Annotation)
}

/// The variable a name (`n`) or a field access on `this` (`this.n`) refers to, with `ancestors` the trees enclosing
/// `expression`, outermost first. `None` if the declaration is not found in the file, or if the name may be a field
/// inherited by an enclosing class (which has a supertype and does not declare the name itself).
pub(super) fn variable_of<'t>(expression: &'t Tree, ancestors: &[&'t Tree], context: &Context) -> Option<Variable<'t>> {
    let expression = strip_parens(expression);
    match &expression.kind {
        K::Ident(name) if name != "this" && name != "super" => {
            let mut child = expression;
            for &parent in ancestors.iter().rev() {
                if let Some(found) = declaration_in(parent, child, name, context) {
                    return found;
                }
                child = parent;
            }
            None
        }
        K::MemberSelect { expr, name } if matches!(&expr.kind, K::Ident(this) if this == "this") => {
            // The innermost class declares the field, or it may be inherited.
            let mut child = expression;
            for &parent in ancestors.iter().rev() {
                let is_child = |t: &Tree| std::ptr::eq(t, child);
                match &parent.kind {
                    K::Class(class) if class.members.iter().any(is_child) => {
                        return declaration_in(parent, child, name, context).flatten();
                    }
                    K::NewClass { body: Some(body), .. } if body.iter().any(is_child) => {
                        return declaration_in(parent, child, name, context).flatten();
                    }
                    _ => {}
                }
                child = parent;
            }
            None
        }
        _ => None,
    }
}

/// [`variable_of`] for an expression below `root`, the tree being checked.
pub(super) fn variable_below<'t>(root: &'t Tree, expression: &'t Tree, context: &Context<'t>) -> Option<Variable<'t>> {
    let mut ancestors = context.ancestors.clone();
    ancestors.push(root);
    let path = path_to(root, expression.start..expression.end);
    let target = path.iter().rposition(|t| std::ptr::eq(*t, expression))?;
    ancestors.extend(&path[..target]);
    variable_of(expression, &ancestors, context)
}

/// The declaration of `name` that `parent` makes visible to its child `child`: `Some(None)` if the name may be
/// inherited, so that the search stops there.
fn declaration_in<'t>(parent: &'t Tree, child: &Tree, name: &str, context: &Context) -> Option<Option<Variable<'t>>> {
    let before = |s: &&Tree| s.end <= child.start;
    let is_child = |t: &Tree| std::ptr::eq(t, child);
    let local = |var: &'t VarDecl| Some(Variable { decl: var, owner: None, ty: local_type(var) });
    match &parent.kind {
        K::Block { stmts, .. } => {
            stmts.iter().filter(before).filter_map(|s| declared_in(s, name)).next_back().map(local)
        }
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => cases
            .iter()
            .flat_map(|c| &c.stmts)
            .filter(before)
            .filter_map(|s| declared_in(s, name))
            .next_back()
            .map(local),
        K::For { init, .. } => init.iter().find_map(|s| declared_in(s, name)).map(local),
        K::ForEach { var, .. } if var.name == name => Some(local(var)),
        K::Try { resources, catches, .. } => {
            let catch = catches.iter().find(|c| c.start <= child.start && child.end <= c.block.end);
            if let Some(catch) = catch.filter(|c| c.param.name == name) {
                // A multi-catch parameter has the union type.
                let ty = catch.param.ty.as_deref().filter(|_| catch.union.is_empty());
                return Some(Some(Variable { decl: &catch.param, owner: None, ty }));
            }
            let resource = |r: &'t Tree| match &r.kind {
                K::Var(var) if var.name == name => Some(&**var),
                _ => None,
            };
            let visible = if resources.iter().any(is_child) { child.start } else { usize::MAX };
            resources.iter().filter(|r| r.end <= visible).find_map(resource).map(local)
        }
        K::Method(method) => method.params.iter().find(|p| p.name == name).map(|p| Some(parameter(p, None, context))),
        K::Lambda { params, .. } => params.iter().find(|p| p.name == name).map(|p| Some(parameter(p, None, context))),
        K::Class(class) => {
            let constant_body =
                class.enum_constants.iter().find_map(|c| c.body.as_deref().filter(|b| b.iter().any(is_child)));
            if let Some(body) = constant_body {
                // The body of an enum constant is an anonymous subclass of the enum, which may declare the name.
                return Some(field_in(body, name, ClassKind::Class));
            }
            if class.kind == ClassKind::Annotation || !class.members.iter().any(is_child) {
                return None;
            }
            let components = if class.kind == ClassKind::Record { &class.record_components[..] } else { &[] };
            let found = field_in(&class.members, name, class.kind).or_else(|| {
                components.iter().find(|c| c.name == name).map(|c| parameter(c, Some(class.kind), context))
            });
            let may_inherit = match class.kind {
                ClassKind::Interface => !class.implements.is_empty(),
                _ => class.extends.is_some() || !class.implements.is_empty(),
            };
            if found.is_some() || may_inherit { Some(found) } else { None }
        }
        // An anonymous class may inherit the name.
        K::NewClass { body: Some(body), .. } if body.iter().any(is_child) => {
            Some(field_in(body, name, ClassKind::Class))
        }
        _ => None,
    }
}

/// The field `name` declared in the body of the class (not inherited).
pub(super) fn declared_field<'t>(class: &'t ClassDecl, name: &str) -> Option<Variable<'t>> {
    field_in(&class.members, name, class.kind)
}

/// The field `name` declared among the members of a class body.
fn field_in<'t>(members: &'t [Tree], name: &str, owner: ClassKind) -> Option<Variable<'t>> {
    let var = members.iter().find_map(|m| declared_in(m, name))?;
    Some(Variable { decl: var, owner: Some(owner), ty: without_dimensions(var) })
}

/// The variable `name` of a local variable or field declaration.
fn declared_in<'t>(declaration: &'t Tree, name: &str) -> Option<&'t VarDecl> {
    let K::Vars(vars) = &declaration.kind else { return None };
    vars.iter().find(|v| v.name == name)
}

/// A parameter or record component, without a type if it has variable arity.
fn parameter<'t>(param: &'t VarDecl, owner: Option<ClassKind>, context: &Context) -> Variable<'t> {
    let is_variable_arity = context.tokens().previous(param.name_pos).is_some_and(|t| &context.text[t] == "...");
    let ty = if is_variable_arity { None } else { without_dimensions(param) };
    Variable { decl: param, owner, ty }
}

/// The type of a local variable: the class created by the initializer for `var`.
fn local_type(var: &VarDecl) -> Option<&Tree> {
    match (&var.ty, var.init.as_deref().map(strip_parens)) {
        (None, Some(Tree { kind: K::NewClass { ident, body: None, .. }, .. })) => Some(ident),
        _ => without_dimensions(var),
    }
}

/// The declared type, unless it has extra array dimensions after the name.
fn without_dimensions(var: &VarDecl) -> Option<&Tree> {
    var.ty.as_deref().filter(|ty| ty.end <= var.name_pos)
}

/// The type of an expression if it is known locally: a class instance creation, a cast, or a variable declared in the
/// file (see [`variable_of`]).
pub(super) fn known_type<'t>(expression: &'t Tree, ancestors: &[&'t Tree], context: &Context) -> Option<&'t Tree> {
    match &strip_parens(expression).kind {
        K::NewClass { ident, .. } => Some(ident),
        K::Cast { ty, .. } => Some(ty),
        _ => variable_of(expression, ancestors, context)?.ty,
    }
}

/// Whether the type of the expression is known locally to be one of the classes (canonical names).
pub(super) fn has_type<'t>(expression: &'t Tree, ancestors: &[&'t Tree], classes: &[&str], context: &Context) -> bool {
    known_type(expression, ancestors, context).is_some_and(|ty| classes.iter().any(|c| names_type(ty, c, context)))
}

/// Whether the type as written names the class with the canonical name `class` (`java.lang.Thread`,
/// `java.util.concurrent.locks.ReentrantReadWriteLock.ReadLock`): written in full, or starting with a simple name
/// that the file's imports resolve to that class or an enclosing one. A type declared in the file with the simple
/// name (or a type parameter) shadows the imports.
pub(super) fn names_type(ty: &Tree, class: &str, context: &Context) -> bool {
    let Some(written) = qualified_name(ty) else { return false };
    if written == class {
        return true;
    }
    let Some(prefix) = class.strip_suffix(written.as_str()).and_then(|p| p.strip_suffix('.')) else { return false };
    let simple = written.split('.').next().unwrap_or_default();
    resolves_simple_name(simple, &format!("{prefix}.{simple}"), context)
}

/// Whether the simple type name refers to the class `class` (a canonical name) in this file.
fn resolves_simple_name(simple: &str, class: &str, context: &Context) -> bool {
    let Some(unit) = context.ancestors.first() else { return false };
    let K::CompilationUnit { package, imports, .. } = &unit.kind else { return false };
    let enclosing = class.rsplit_once('.').map_or("", |(p, _)| p);
    let mut on_demand = enclosing == "java.lang"
        || package.as_ref().and_then(|(_, name)| qualified_name(name)).is_some_and(|p| p == enclosing);
    for import in imports {
        let K::Import { name, .. } = &import.kind else { continue };
        let Some(imported) = qualified_name(name) else { continue };
        match imported.strip_suffix(".*") {
            Some(package) => on_demand |= package == enclosing,
            None if imported.rsplit('.').next() == Some(simple) => {
                return imported == class && !declares_type(unit, simple);
            }
            None => {}
        }
    }
    on_demand && !declares_type(unit, simple)
}

/// Whether the file declares a class or type parameter with the simple name.
fn declares_type(unit: &Tree, simple: &str) -> bool {
    let mut found = false;
    for_each_descendant(unit, &mut |tree, _| {
        found |= match &tree.kind {
            K::Class(class) => class.name == simple,
            K::TypeParameter { name, .. } => name == simple,
            _ => false,
        };
        !found
    });
    found
}

/// The qualified name of a class type without type arguments and annotations (`a.b.C<D>` is `a.b.C`, an import's
/// `a.b.*` is `a.b.*`); `None` for other types.
fn qualified_name(tree: &Tree) -> Option<String> {
    match &tree.kind {
        K::Ident(name) => Some(name.clone()),
        K::MemberSelect { expr, name } => Some(format!("{}.{name}", qualified_name(expr)?)),
        K::Parameterized { base, .. } | K::Annotated { base, .. } => qualified_name(base),
        _ => None,
    }
}

/// The receiver and the name of a method invocation (`None` as receiver for an unqualified call).
pub(super) fn method_call(invocation: &Tree) -> Option<(Option<&Tree>, &str, &[Tree])> {
    let K::MethodInvocation { select, args, .. } = &invocation.kind else { return None };
    match &select.kind {
        K::Ident(name) => Some((None, name, args)),
        K::MemberSelect { expr, name } => Some((Some(expr), name, args)),
        _ => None,
    }
}

/// The trees enclosing the checked tree in the same code, innermost first, each with its child towards the checked
/// tree: up to the enclosing method, lambda, class body or initializer (exclusive, see [`code_boundary`]).
pub(super) fn enclosing_in_same_code<'c, 't>(
    tree: &'t Tree,
    context: &'c Context<'t>,
) -> impl Iterator<Item = (&'t Tree, &'t Tree)> + 'c {
    let mut child = tree;
    context.ancestors.iter().rev().map_while(move |&parent| {
        let pair = (parent, child);
        child = parent;
        (!is_boundary(pair.0, pair.1)).then_some(pair)
    })
}

/// The tree whose code contains the checked tree: the enclosing method, lambda, class (for initializers) or anonymous
/// class.
pub(super) fn code_boundary<'t>(tree: &'t Tree, context: &Context<'t>) -> Option<&'t Tree> {
    let mut child = tree;
    for &parent in context.ancestors.iter().rev() {
        if is_boundary(parent, child) {
            return Some(parent);
        }
        child = parent;
    }
    None
}

fn is_boundary(parent: &Tree, child: &Tree) -> bool {
    match &parent.kind {
        K::Method(_) | K::Lambda { .. } | K::Class(_) => true,
        K::NewClass { body: Some(body), .. } => body.iter().any(|m| std::ptr::eq(m, child)),
        _ => false,
    }
}

/// Calls `f` on each descendant of `tree` in the same code (not in lambdas and class bodies, whose code runs in a
/// different context) in source order, with its parent; `f` returns whether to visit the descendant's children too.
/// Annotations are left out.
pub(super) fn for_each_in_same_code<'t>(tree: &'t Tree, f: &mut impl FnMut(&'t Tree, &'t Tree) -> bool) {
    for_each_descendant(tree, &mut |child, parent| {
        let in_class_body = match &parent.kind {
            K::NewClass { body: Some(body), .. } => body.iter().any(|m| std::ptr::eq(m, child)),
            _ => false,
        };
        let other_code = matches!(child.kind, K::Lambda { .. } | K::Class(_) | K::Annotation { .. });
        !in_class_body && !other_code && f(child, parent)
    });
}

/// The source of an expression without whitespace and comments, for comparing expressions.
pub(super) fn normalized(tree: &Tree, context: &Context) -> String {
    context.tokens().in_range(tree.start..tree.end).map(|t| &context.text[t]).collect()
}

pub(super) const THREAD: &str = "java.lang.Thread";
pub(super) const CONDITION: &str = "java.util.concurrent.locks.Condition";

/// Whether the expression is known locally to be a `Thread`: see [`has_type`], or `Thread.currentThread()`.
pub(super) fn is_thread<'t>(expression: &'t Tree, ancestors: &[&'t Tree], context: &Context) -> bool {
    let is_current_thread = match method_call(strip_parens(expression)) {
        Some((Some(receiver), "currentThread", [])) => names_type(receiver, THREAD, context),
        _ => false,
    };
    is_current_thread || has_type(expression, ancestors, &[THREAD], context)
}
