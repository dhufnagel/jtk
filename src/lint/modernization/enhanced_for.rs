//! M007: `for` loop over the indexes of an array or list that can be an enhanced `for` loop (PMD
//! ForLoopCanBeForeach, Eclipse save action "Convert to enhanced for loop"), Java 5.
//!
//! As in PMD: `for (int i = 0; i < a.length; i++)` (also `i <= a.length - 1`, `++i`, `i += 1`, `i = i + 1`) where
//! the body uses `i` only to read `a[i]`, or `list.get(i)` with `i < list.size()`; and loops over an iterator
//! (`for (Iterator<T> it = c.iterator(); it.hasNext();)`) that use it only for `it.next()`. Without type information
//! the array or list must be a variable, parameter or field declared in the file with an array type or a `List` type
//! of `java.util`; PMD also knows inherited fields and other `List` types.
//!
//! The fix turns an index loop into `for (T e : a)` if the element type `T` is known (the array's element type, the
//! list's type argument) and the body does not use the array or list otherwise (so the array cannot change, nor the
//! list be modified, which would make the iterator throw). The element is named after the array or list (`items`:
//! `item`), or `element` if that name is taken or not a plural.

use std::ops::Range;

use crate::ast::{CaseKind, ClassKind, K, Tree, VarDecl};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};

use super::shared_a::for_each_variable_reference;

const LIST_TYPES: &[&str] = &[
    "List",
    "ArrayList",
    "LinkedList",
    "Vector",
    "Stack",
    "CopyOnWriteArrayList",
    "AbstractList",
    "AbstractSequentialList",
];

/// The `java.util` types whose iterator loops are reported (and `Iterable`).
const COLLECTION_TYPES: &[&str] = &[
    "Iterable",
    "Collection",
    "List",
    "ArrayList",
    "LinkedList",
    "Vector",
    "Stack",
    "Set",
    "HashSet",
    "LinkedHashSet",
    "TreeSet",
    "SortedSet",
    "NavigableSet",
    "Queue",
    "Deque",
    "ArrayDeque",
    "PriorityQueue",
];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Array,
    List,
}

/// The parts of the checked `for` loop.
struct Loop<'t> {
    tree: &'t Tree,
    init: Option<&'t Tree>,
    cond: Option<&'t Tree>,
    update: &'t [Tree],
    body: &'t Tree,
}

pub fn check<'t>(tree: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let K::For { init, cond, update, body } = &tree.kind else { return };
    let message = "This 'for' loop can be replaced by an enhanced 'for' loop";
    let for_loop = Loop { tree, init: init.first(), cond: cond.as_deref(), update, body };
    let Some(init) = for_loop.init else {
        // `int i = 0; for (; i < a.length; i++)`: reported, not fixed.
        if index_loop(&for_loop, None, context).is_some() {
            diagnostics.push(Diagnostic::new(Rule::EnhancedFor, tree.start, message));
        }
        return;
    };
    let K::Vars(vars) = &init.kind else { return };
    let [var] = vars.as_slice() else { return };
    // `var` has no type.
    let (Some(value), Some(index_type)) = (var.init.as_deref(), var.ty.as_deref()) else { return };
    if matches!(&index_type.kind, K::Primitive(p) if p == "int") && context.source(value) == "0" {
        let Some(fix) = index_loop(&for_loop, Some(&var.name), context) else { return };
        let diagnostic = Diagnostic::new(Rule::EnhancedFor, tree.start, message);
        diagnostics.push(match fix {
            Some(edits) => diagnostic.with_fix(edits),
            None => diagnostic,
        });
    } else if is_iterator_loop(&for_loop, &var.name, index_type, value, context) {
        diagnostics.push(Diagnostic::new(Rule::EnhancedFor, tree.start, message));
    }
}

/// Whether the loop iterates over the indexes of an array or list (`None` if not), with the fix if there is one.
/// Without the index declared in the loop (`index` is `None`), it must be an `int` variable declared in the block
/// around the loop, and used only in the loop.
fn index_loop(for_loop: &Loop, index: Option<&str>, context: &Context) -> Option<Option<Vec<Edit>>> {
    let fixable = index.is_some();
    let (index, scope) = match index {
        Some(index) => (index, for_loop.body),
        None => {
            let index = incremented_variable(for_loop, context.text)?;
            let block = context.parent()?;
            let K::Block { stmts, .. } = &block.kind else { return None };
            let declared_int = stmts
                .iter()
                .take_while(|s| s.start < for_loop.tree.start)
                .filter_map(|s| variable(s, index))
                .last()
                .is_some_and(|v| v.ty.as_deref().is_some_and(|t| matches!(&t.kind, K::Primitive(p) if p == "int")));
            if !declared_int {
                return None;
            }
            (index, block)
        }
    };
    let (iterable, kind) = iterable_of_condition(for_loop, index, context)?;
    if incremented_variable(for_loop, context.text) != Some(index) {
        return None;
    }
    let accesses = element_accesses(for_loop, scope, index, &iterable, kind, context)?;
    let declared = declared_type(for_loop.tree, &context.ancestors, &iterable)?;
    let element_type = element_type(&declared, kind, context)?;
    if kind == Kind::List && !declared.ty.is_some_and(|t| is_java_util_type(t, context.text)) {
        return None;
    }
    Some(element_type.filter(|_| fixable).and_then(|t| enhanced_for(for_loop, &iterable, &t, &accesses, context)))
}

/// The variable of the name if the statement is a local variable declaration that declares it.
fn variable<'t>(statement: &'t Tree, name: &str) -> Option<&'t VarDecl> {
    match &statement.kind {
        K::Vars(vars) => vars.iter().find(|v| v.name == name),
        _ => None,
    }
}

/// The iterated array or list: a name or `this.name`.
struct Iterable<'a> {
    name: &'a str,
    qualified: bool,
    code: &'a str,
}

impl<'a> Iterable<'a> {
    /// The array, list or collection that the expression names, if it is a name or `this.name`.
    fn of(expression: &'a Tree, text: &'a str) -> Option<Iterable<'a>> {
        let code = &text[expression.start..expression.end];
        match &expression.kind {
            K::Ident(name) if name != "this" && name != "super" => Some(Iterable { name, qualified: false, code }),
            K::MemberSelect { expr, name } if matches!(&expr.kind, K::Ident(this) if this == "this") => {
                Some(Iterable { name, qualified: true, code })
            }
            _ => None,
        }
    }
}

/// The array or list of the condition `i < a.length`, `i <= a.length - 1` or `i < list.size()`.
fn iterable_of_condition<'t>(for_loop: &Loop<'t>, index: &str, context: &Context<'t>) -> Option<(Iterable<'t>, Kind)> {
    let K::Binary { op, lhs, rhs } = &for_loop.cond?.kind else { return None };
    if !matches!(&lhs.kind, K::Ident(name) if name == index) {
        return None;
    }
    let bound = match op.as_str() {
        "<" => rhs,
        "<=" => match &rhs.kind {
            K::Binary { op, lhs, rhs: one } if op == "-" && context.source(one) == "1" => lhs,
            _ => return None,
        },
        _ => return None,
    };
    let (object, kind) = match &bound.kind {
        K::MemberSelect { expr, name } if name == "length" => (expr, Kind::Array),
        K::MethodInvocation { select, type_args, args } if type_args.is_empty() && args.is_empty() => {
            match &select.kind {
                K::MemberSelect { expr, name } if name == "size" => (expr, Kind::List),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some((Iterable::of(object, context.text)?, kind))
}

/// The variable that the loop's only update increments by one.
fn incremented_variable<'t>(for_loop: &Loop<'t>, text: &str) -> Option<&'t str> {
    let [update] = for_loop.update else { return None };
    let name = |tree: &'t Tree| match &tree.kind {
        K::Ident(name) => Some(name.as_str()),
        _ => None,
    };
    match &update.kind {
        K::Unary { op, expr, .. } if op == "++" => name(expr),
        K::Assign { op, lhs, rhs } if op == "+=" && is_one(rhs, text) => name(lhs),
        K::Assign { op, lhs, rhs } if op == "=" => {
            let variable = name(lhs)?;
            let K::Binary { op, lhs: a, rhs: b } = &rhs.kind else { return None };
            let is_variable = |tree: &Tree| matches!(&tree.kind, K::Ident(n) if n == variable);
            let incremented = is_variable(a) && is_one(b, text) || is_one(a, text) && is_variable(b);
            (op == "+" && incremented).then_some(variable)
        }
        _ => None,
    }
}

/// Whether the expression is `1`.
fn is_one(tree: &Tree, text: &str) -> bool {
    &text[tree.start..tree.end] == "1"
}

/// Whether the type is a type of `java.util`: written qualified, or imported by name or with `*`.
fn is_java_util_type(ty: &Tree, text: &str) -> bool {
    let base = match &ty.kind {
        K::Parameterized { base, .. } => base,
        _ => ty,
    };
    let base = &text[base.start..base.end];
    if let Some(simple) = base.strip_prefix("java.util.") {
        return !simple.contains('.');
    }
    !base.contains('.') && (text.contains("import java.util.*;") || text.contains(&format!("import java.util.{base};")))
}

/// The element accesses `a[i]` or `list.get(i)` in the body, if they are the only uses of the index in `scope` and
/// only read the element.
fn element_accesses<'t>(
    for_loop: &Loop<'t>,
    scope: &'t Tree,
    index: &str,
    iterable: &Iterable,
    kind: Kind,
    context: &Context<'t>,
) -> Option<Vec<&'t Tree>> {
    let range = |tree: &Tree| tree.start..tree.end;
    let body = range(for_loop.body);
    // In the scope of an index declared before the loop, its uses in the loop's condition and update.
    let condition = range(for_loop.cond?);
    let update = range(for_loop.update.first()?);
    let mut accesses = Vec::new();
    let mut valid = true;
    for_each_variable_reference(scope, &mut |identifier, path| {
        let position = identifier.start;
        if !matches!(&identifier.kind, K::Ident(name) if name == index)
            || condition.contains(&position)
            || update.contains(&position)
        {
            return true;
        }
        let parent = path[path.len() - 1];
        let grandparent = path.len().checked_sub(2).map(|i| path[i]);
        match element_access(identifier, parent, grandparent, iterable, kind, context) {
            Some(access) if body.contains(&position) => accesses.push(access),
            _ => valid = false,
        }
        valid
    });
    valid.then_some(accesses)
}

/// The access `a[i]` or `list.get(i)` of the index identifier, if it reads the element.
fn element_access<'t>(
    index: &Tree,
    parent: &'t Tree,
    grandparent: Option<&Tree>,
    iterable: &Iterable,
    kind: Kind,
    context: &Context,
) -> Option<&'t Tree> {
    let is = |a: &Tree, b: &Tree| std::ptr::eq(a, b);
    match (kind, &parent.kind) {
        (Kind::Array, K::ArrayAccess { expr, index: i }) => {
            let is_access = is(i, index) && context.source(expr) == iterable.code;
            let is_written = grandparent.is_some_and(|g| match &g.kind {
                K::Assign { lhs, .. } => is(lhs, parent),
                K::Unary { op, .. } => op == "++" || op == "--",
                _ => false,
            });
            (is_access && !is_written).then_some(parent)
        }
        (Kind::List, K::MethodInvocation { select, args, .. }) => {
            let K::MemberSelect { expr, name } = &select.kind else { return None };
            let is_get = matches!(args.as_slice(), [argument] if is(argument, index))
                && name == "get"
                && context.source(expr) == iterable.code
                && !context.tokens().has_comment(select.end..parent.end);
            is_get.then_some(parent)
        }
        _ => None,
    }
}

/// The declared type of the array or list: the declaration of a local variable, parameter or field in the file.
struct Declared<'t> {
    /// With the dimensions after the name (`int a[]`) and of a variable arity parameter; `None` for `var`.
    ty: Option<&'t Tree>,
    name_pos: usize,
}

impl<'t> Declared<'t> {
    fn of(var: &'t VarDecl) -> Declared<'t> {
        Declared { ty: var.ty.as_deref(), name_pos: var.name_pos }
    }
}

/// The declaration of the iterable seen from the loop, whose enclosing trees are `ancestors`: the innermost local
/// variable or parameter declared before it, or field of the innermost class.
fn declared_type<'t>(loop_tree: &'t Tree, ancestors: &[&'t Tree], iterable: &Iterable) -> Option<Declared<'t>> {
    let name = iterable.name;
    let local = !iterable.qualified;
    let before = |statements: &'t [Tree], child: &Tree| {
        statements.iter().take_while(|s| s.start < child.start).find_map(|s| variable(s, name)).map(Declared::of)
    };
    let field = |members: &'t [Tree]| members.iter().find_map(|m| variable(m, name)).map(Declared::of);
    let mut child = loop_tree;
    for &tree in ancestors.iter().rev() {
        let contains = |trees: &[Tree]| trees.iter().any(|t| std::ptr::eq(t, child));
        // A class body ends the search for `this.name`.
        let found = match &tree.kind {
            K::Block { stmts, .. } if local => before(stmts, child),
            K::Switch { cases, .. } | K::SwitchExpr { cases, .. } if local => cases
                .iter()
                .find(|c| c.kind == CaseKind::Statement && contains(&c.stmts))
                .and_then(|c| before(&c.stmts, child)),
            K::For { init, .. } if local => init.first().and_then(|i| variable(i, name)).map(Declared::of),
            K::ForEach { var, .. } if local && var.name == name => Some(Declared::of(var)),
            K::Method(method) if local && !method.compact_ctor => {
                method.params.iter().find(|p| p.name == name).map(Declared::of)
            }
            // Only explicitly typed lambda parameters.
            K::Lambda { params, .. } if local && params.iter().any(|p| p.ty.is_some()) => {
                params.iter().find(|p| p.name == name).map(Declared::of)
            }
            K::Class(class) => {
                let body = match class.kind {
                    ClassKind::Class | ClassKind::Record => Some(&class.members[..]),
                    ClassKind::Enum if contains(&class.members) => Some(&class.members[..]),
                    ClassKind::Enum => {
                        class.enum_constants.iter().filter_map(|c| c.body.as_deref()).find(|b| contains(b))
                    }
                    ClassKind::Interface | ClassKind::Annotation => None,
                };
                match body.map(field) {
                    Some(found) if found.is_some() || !local => return found,
                    _ => None,
                }
            }
            K::NewClass { body: Some(body), .. } if contains(body) => match field(body) {
                found if found.is_some() || !local => return found,
                _ => None,
            },
            _ => None,
        };
        if found.is_some() {
            return found;
        }
        child = tree;
    }
    None
}

/// Whether the declared type is an array or list type (`None` if not); with the element type if it is known.
fn element_type(declared: &Declared, kind: Kind, context: &Context) -> Option<Option<String>> {
    let ty = declared.ty?;
    match kind {
        Kind::Array => {
            let mut element = ty;
            let mut dimensions = 0;
            while let K::ArrayType { elem, dims } = &element.kind {
                dimensions += dims.len();
                element = elem;
            }
            if dimensions == 0 {
                return None;
            }
            // The declared type, without the dimensions after the name.
            let annotated = context.text[ty.start..ty.end.min(declared.name_pos)].contains('@');
            let element = format!("{}{}", context.source(element), "[]".repeat(dimensions - 1));
            Some((!annotated).then_some(element))
        }
        Kind::List => {
            let (base, arguments) = match &ty.kind {
                K::Parameterized { base, args } => (&**base, Some(args)),
                K::Ident(_) | K::MemberSelect { .. } => (ty, None),
                _ => return None,
            };
            let has_comment = context.tokens().has_comment(base.end..ty.end);
            let base = context.source(base);
            let simple = base.rsplit('.').next().unwrap_or(base);
            let is_list = LIST_TYPES.contains(&simple) && (simple == base || base.starts_with("java.util."));
            if !is_list {
                return None;
            }
            let argument = arguments.and_then(|a| match a.as_slice() {
                [argument] if !matches!(argument.kind, K::Wildcard { .. }) && !has_comment => {
                    Some(context.source(argument).to_string())
                }
                _ => None,
            });
            Some(argument)
        }
    }
}

/// The edits that turn the loop into `for (T e : iterable)`, if the body uses the iterable only in the element
/// accesses.
fn enhanced_for(
    for_loop: &Loop,
    iterable: &Iterable,
    element_type: &str,
    accesses: &[&Tree],
    context: &Context,
) -> Option<Vec<Edit>> {
    let text = context.text;
    let tokens = context.tokens();
    let body = for_loop.body;
    let inside = |range: &Range<usize>, tree: &Tree| tree.start <= range.start && range.end <= tree.end;
    let other_use = tokens
        .in_range(body.start..body.end)
        .any(|t| &text[t.clone()] == iterable.name && !accesses.iter().any(|a| inside(&t, a)));
    let open = tokens.next(for_loop.tree.start + "for".len())?;
    let close = tokens.previous(body.start)?;
    let header = open.start..close.end;
    // Comments in the condition, or in the header but not in the initialization or the update.
    let in_part = |comment: &Range<usize>| for_loop.init.into_iter().chain(for_loop.update).any(|t| inside(comment, t));
    if other_use || tokens.comments_in(header.clone()).any(|c| !in_part(&c)) {
        return None;
    }
    let name = element_name(iterable.name, text);
    // One edit for the whole loop, so that a nested loop is converted in a later round, when this loop's element
    // name is in the text.
    let mut replacements = vec![(header, format!("({element_type} {name} : {})", iterable.code))];
    replacements.extend(accesses.iter().map(|a| (a.start..a.end, name.clone())));
    let mut code = String::new();
    let mut position = for_loop.tree.start;
    for (range, replacement) in replacements {
        code.push_str(&text[position..range.start]);
        code.push_str(&replacement);
        position = range.end;
    }
    code.push_str(&text[position..for_loop.tree.end]);
    Some(vec![Edit::new(for_loop.tree.start..for_loop.tree.end, code)])
}

/// The singular of the iterable's name, or `element`; with a number if the name occurs in the file.
fn element_name(iterable: &str, text: &str) -> String {
    let singular = if let Some(stem) = iterable.strip_suffix("ies") {
        format!("{stem}y")
    } else if ["sses", "xes", "ches", "shes"].iter().any(|s| iterable.ends_with(s)) {
        iterable[..iterable.len() - 2].to_string()
    } else if iterable.ends_with('s') && !iterable.ends_with("ss") {
        iterable[..iterable.len() - 1].to_string()
    } else {
        String::new()
    };
    let is_free = |name: &str| !name.is_empty() && !JAVA_KEYWORDS.contains(&name) && !occurs_as_word(name, text);
    if singular.len() > 1 && is_free(&singular) {
        return singular;
    }
    (1..).map(|n| if n == 1 { "element".to_string() } else { format!("element{n}") }).find(|n| is_free(n)).unwrap()
}

/// Whether the name occurs in the text as a whole word: then it may be in scope.
fn occurs_as_word(name: &str, text: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(name).any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(is_word)
            && !text[i + name.len()..].chars().next().is_some_and(is_word)
    })
}

const JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    "true",
    "false",
    "null",
    "var",
    "yield",
    "record",
    "sealed",
    "permits",
];

/// `for (Iterator<T> it = c.iterator(); it.hasNext();)` that uses `it` only in the condition and one `it.next()`.
fn is_iterator_loop(for_loop: &Loop, iterator: &str, iterator_type: &Tree, value: &Tree, context: &Context) -> bool {
    let base = match &iterator_type.kind {
        K::Parameterized { base, .. } => base,
        _ => iterator_type,
    };
    let is_iterator_type = matches!(context.source(base), "Iterator" | "java.util.Iterator");
    let condition_is_has_next = for_loop
        .cond
        .is_some_and(|c| is_call(c, "hasNext", context) && receiver(c).is_some_and(|r| context.source(r) == iterator));
    if !is_iterator_type || !is_call(value, "iterator", context) || !condition_is_has_next {
        return false;
    }
    // PMD needs to know that the receiver is an `Iterable`.
    let Some(collection) = receiver(value).filter(|r| !matches!(&r.kind, K::Ident(name) if name == "this")) else {
        // `iterator()` of the class itself.
        let class = context.ancestors.iter().rev().find_map(|t| match &t.kind {
            K::Class(class) if class.kind == ClassKind::Class => Some(class),
            _ => None,
        });
        return class.is_some_and(|c| match (c.implements.first(), c.implements.last()) {
            (Some(first), Some(last)) => context.text[first.start..last.end].contains("Iterable"),
            _ => false,
        });
    };
    let Some(collection) = Iterable::of(collection, context.text) else { return false };
    let is_collection = declared_type(for_loop.tree, &context.ancestors, &collection).is_some_and(|d| {
        let Some(ty) = d.ty else { return false };
        let base = match &ty.kind {
            K::Parameterized { base, .. } => base,
            K::ArrayType { .. } => return false,
            _ => ty,
        };
        let base = context.source(base);
        COLLECTION_TYPES.contains(&base.rsplit('.').next().unwrap_or(base))
            && (is_java_util_type(ty, context.text) || matches!(base, "Iterable" | "java.lang.Iterable"))
    });
    if !is_collection {
        return false;
    }
    let mut uses = 0;
    let mut next_calls = 0;
    for_each_variable_reference(for_loop.tree, &mut |identifier, path| {
        if matches!(&identifier.kind, K::Ident(name) if name == iterator) {
            uses += 1;
            if let [.., call, _] = path
                && is_call(call, "next", context)
                && receiver(call).is_some_and(|r| std::ptr::eq(r, identifier))
            {
                next_calls += 1;
            }
        }
        true
    });
    uses == 2 && next_calls == 1
}

/// Whether the expression calls the method without arguments.
fn is_call(tree: &Tree, method: &str, context: &Context) -> bool {
    let K::MethodInvocation { select, args, .. } = &tree.kind else { return false };
    let name = match &select.kind {
        K::Ident(name) | K::MemberSelect { name, .. } => name,
        _ => return false,
    };
    name == method && args.is_empty() && !context.tokens().has_comment(select.end..tree.end)
}

/// The object a method is called on: `a` in `a.m()`.
fn receiver(call: &Tree) -> Option<&Tree> {
    match &call.kind {
        K::MethodInvocation { select, .. } => match &select.kind {
            K::MemberSelect { expr, .. } => Some(expr),
            _ => None,
        },
        _ => None,
    }
}
