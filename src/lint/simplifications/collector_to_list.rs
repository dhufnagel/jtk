//! S155: `stream.collect(Collectors.toList())` or `collect(Collectors.toUnmodifiableList())` instead of
//! `stream.toList()` (Picnic CollectorMutability, Sonar S6204, IntelliJ SimplifyCollector): since Java 16, `Stream`
//! has `toList()`, which is shorter and states that the list is not to be changed.
//!
//! Unsafe fix: `stream.toList()`. Its list is unmodifiable, while that of `Collectors.toList()` is in practice a
//! mutable `ArrayList` that the code may change later; and unlike `Collectors.toUnmodifiableList()`, it accepts null
//! elements. The collector takes its element type from the target (`List<Object> l = strings.collect(toList())`
//! compiles, `strings.toList()` does not), so the fix is offered only where the call has no target type (`var`, a
//! statement, the receiver of a call, the `for` loop's expression) or the target is a `List<E>` declared in the file
//! (the variable assigned or initialized, or the method returned from) and the stream's elements are locally known
//! to be `E`: `.map(E::new)`, or `list.stream()` of a local variable, parameter or field declared as a collection of
//! `E`, followed by calls that keep the elements (`filter`, `sorted`, ...). The only gap: `.map(E::new)` of a generic
//! `E` collected into a raw `List<E>`. No fix with type arguments on either call or comments in the replaced range.
//!
//! Limits: without types the receiver is not known to be a `Stream`; `collect` with a `java.util.stream.Collector`
//! argument on anything else is very unlikely. `Collectors` must be `java.util.stream.Collectors` by its qualified
//! name or through the imports, or `toList` statically imported from it (by name or on demand, while the file
//! declares no method of that name). `Collectors.toSet()` is not reported, as `Stream` has no `toSet()`.

use super::shared_batch3::without_whitespace;
use crate::lint::logging::shared::{Declared, declaration, imports, names_type, qualified_name};
use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree, VarDecl};

const COLLECTORS: &str = "java.util.stream.Collectors";

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { select, type_args, args } = &tree.kind else { return };
    let K::MemberSelect { name, .. } = &select.kind else { return };
    let [collector] = args.as_slice() else { return };
    let Some(unit) = context.ancestors.first() else { return };
    let K::MethodInvocation { select: factory, type_args: factory_type_args, args: factory_args } = &collector.kind
    else {
        return;
    };
    let factory_name = match &factory.kind {
        K::MemberSelect { expr, name } if names_type(expr, COLLECTORS, unit) => name,
        K::Ident(name) if imports_statically(unit, name) => name,
        _ => return,
    };
    if name != "collect"
        || !factory_args.is_empty()
        || !matches!(factory_name.as_str(), "toList" | "toUnmodifiableList")
    {
        return;
    }
    let message = format!("collect(Collectors.{factory_name}()) instead of toList()");
    let diagnostic = Diagnostic::new(Rule::CollectorToList, tree.start, message);
    let replaced = select.end - name.len()..tree.end;
    let K::MemberSelect { expr: stream, .. } = &select.kind else { return };
    let fixable = type_args.is_empty()
        && factory_type_args.is_empty()
        && !context.tokens().has_comment(replaced.clone())
        && keeps_compiling(tree, stream, context);
    diagnostics.push(if fixable {
        diagnostic.with_unsafe_fix(vec![Edit::new(replaced, "toList()")])
    } else {
        diagnostic
    });
}

/// Whether the method `name` is statically imported from `Collectors`, and not shadowed by a method of the file.
fn imports_statically(unit: &Tree, name: &str) -> bool {
    let mut by_name = false;
    let mut on_demand = false;
    for import in imports(unit) {
        let K::Import { is_static: true, name: imported } = &import.kind else { continue };
        let K::MemberSelect { expr: class, name: member } = &imported.kind else { continue };
        let from_collectors = qualified_name(class) == COLLECTORS;
        by_name |= from_collectors && member == name;
        on_demand |= from_collectors && member == "*";
    }
    (by_name || on_demand) && !declares_method(unit, name)
}

fn declares_method(unit: &Tree, name: &str) -> bool {
    let mut found = false;
    for_each_descendant(unit, &mut |tree, _| {
        found |= matches!(&tree.kind, K::Method(method) if method.name == name);
        !found
    });
    found
}

/// The methods of `Stream` that return a stream of the same elements.
const KEEPING_ELEMENTS: &[&str] = &["distinct", "dropWhile", "filter", "limit", "peek", "skip", "sorted", "takeWhile"];

/// The collections whose `stream()` streams their elements.
const COLLECTIONS: &[&str] =
    &["ArrayList", "Collection", "HashSet", "LinkedList", "List", "Set", "SortedSet", "TreeSet"];

/// Whether `stream.toList()` in place of the call `call` compiles: the call has no target type, or the target is a
/// list of the stream's elements.
fn keeps_compiling(call: &Tree, stream: &Tree, context: &Context) -> bool {
    let Some(parent) = context.parent() else { return false };
    let is_call = |t: &Tree| std::ptr::eq(t, call);
    let target = match &parent.kind {
        K::MemberSelect { .. } | K::ExprStmt(_) => return true,
        K::ForEach { expr, .. } => return is_call(expr),
        K::Vars(vars) => match vars.iter().find(|v| v.init.as_deref().is_some_and(is_call)) {
            Some(VarDecl { ty: None, .. }) => return true,
            Some(var) => var.ty.as_deref(),
            None => None,
        },
        K::Return(_) => return_type(context),
        K::Assign { lhs, rhs, .. } if is_call(rhs) => assigned_type(lhs, call, context),
        _ => None,
    };
    let element = target.and_then(|t| collection_element(t, &["List"]));
    element.is_some_and(|e| element_type(stream, call, context) == Some(without_whitespace(context.source(e))))
}

/// The return type of the method the call is returned from, unless it is in a lambda.
fn return_type<'t>(context: &Context<'t>) -> Option<&'t Tree> {
    context.ancestors.iter().rev().find_map(|a| match &a.kind {
        K::Lambda { .. } => Some(None),
        K::Method(method) => Some(method.ret.as_deref()),
        _ => None,
    })?
}

/// The declared type of the variable `lhs`: a name, or a field of `this`.
fn assigned_type<'t>(lhs: &'t Tree, call: &'t Tree, context: &Context<'t>) -> Option<&'t Tree> {
    match &lhs.kind {
        K::Ident(name) => declared_type(call, name, context),
        K::MemberSelect { expr, name } if matches!(&expr.kind, K::Ident(this) if this == "this") => {
            // `this` of an anonymous class is not known.
            let class = context.ancestors.iter().rev().find_map(|a| match &a.kind {
                K::Class(class) => Some(Some(class)),
                K::NewClass { body: Some(_), .. } => Some(None),
                _ => None,
            })??;
            let field = class.members.iter().find_map(|m| match &m.kind {
                K::Vars(vars) => vars.iter().find(|v| v.name == *name),
                _ => None,
            })?;
            field.ty.as_deref().filter(|ty| ty.end <= field.name_pos)
        }
        _ => None,
    }
}

/// The declared type of the variable `name` visible at `tree`.
fn declared_type<'t>(tree: &'t Tree, name: &str, context: &Context<'t>) -> Option<&'t Tree> {
    match declaration(tree, &context.ancestors, name, context)? {
        Declared::Var(var) => var.ty.as_deref(),
        _ => None,
    }
}

/// The element type `E` of a type `C<E>` where `C` is one of the `collections` of `java.util`.
fn collection_element<'t>(ty: &'t Tree, collections: &[&str]) -> Option<&'t Tree> {
    let K::Parameterized { base, args } = &ty.kind else { return None };
    let name = qualified_name(base);
    let simple = name.rsplit('.').next()?;
    let known = collections.contains(&simple) && (name == simple || name == format!("java.util.{simple}"));
    match args.as_slice() {
        [element] if known && !matches!(element.kind, K::Wildcard { .. }) => Some(element),
        _ => None,
    }
}

/// The source of the element type of `stream`, as far as it is locally known.
fn element_type(stream: &Tree, call: &Tree, context: &Context) -> Option<String> {
    let K::MethodInvocation { select, args, type_args } = &stream.kind else { return None };
    let K::MemberSelect { expr: inner, name } = &select.kind else { return None };
    if !type_args.is_empty() {
        return None;
    }
    match (name.as_str(), args.as_slice()) {
        ("map", [reference]) => match &reference.kind {
            // A constructor reference.
            K::MemberRef { qual, type_args, name: None }
                if type_args.is_empty() && matches!(qual.kind, K::Ident(_) | K::MemberSelect { .. }) =>
            {
                Some(without_whitespace(context.source(qual)))
            }
            _ => None,
        },
        ("stream", []) => {
            let K::Ident(collection) = &inner.kind else { return None };
            let ty = declared_type(call, collection, context)?;
            collection_element(ty, COLLECTIONS).map(|e| without_whitespace(context.source(e)))
        }
        (name, _) if KEEPING_ELEMENTS.contains(&name) => element_type(inner, call, context),
        _ => None,
    }
}
