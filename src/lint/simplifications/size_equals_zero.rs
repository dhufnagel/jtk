//! S113: `size()` or `length()` compared with 0 instead of `isEmpty()` (Baseline CardinalityEqualsZero, IntelliJ
//! SizeReplaceableByIsEmpty, NetBeans size() == 0, PMD UseCollectionIsEmpty, Picnic CharSequenceIsEmpty).
//!
//! Without type information, only a variable, parameter or field declared in the file counts as receiver: of a
//! collection or map type of `java.util` (or `java.util.concurrent`), imported or written fully qualified, for
//! `size()`, or of `String` for `length()`. `StringBuilder` and `CharSequence` only have `isEmpty()` since Java 15,
//! so they are left out. The comparisons are `== 0`, `<= 0` and `< 1` (empty) and `!= 0`, `> 0` and `>= 1` (not
//! empty), also with the operands swapped.
//!
//! The fix writes `x.isEmpty()` or `!x.isEmpty()`, which binds tighter than the comparison it replaces and is the same
//! for the JDK's collections, maps and strings.

use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

use super::shared_batch3::{class_name, declared_type, refers_to};

const COLLECTIONS: &[&str] = &[
    "java.util.Collection",
    "java.util.List",
    "java.util.Set",
    "java.util.SortedSet",
    "java.util.NavigableSet",
    "java.util.Queue",
    "java.util.Deque",
    "java.util.Map",
    "java.util.SortedMap",
    "java.util.NavigableMap",
    "java.util.ArrayList",
    "java.util.LinkedList",
    "java.util.HashSet",
    "java.util.LinkedHashSet",
    "java.util.TreeSet",
    "java.util.ArrayDeque",
    "java.util.PriorityQueue",
    "java.util.HashMap",
    "java.util.LinkedHashMap",
    "java.util.TreeMap",
    "java.util.EnumMap",
    "java.util.IdentityHashMap",
    "java.util.WeakHashMap",
    "java.util.Vector",
    "java.util.Stack",
    "java.util.concurrent.ConcurrentMap",
    "java.util.concurrent.ConcurrentHashMap",
    "java.util.concurrent.CopyOnWriteArrayList",
    "java.util.concurrent.CopyOnWriteArraySet",
    "java.util.concurrent.BlockingQueue",
    "java.util.concurrent.LinkedBlockingQueue",
    "java.util.concurrent.ArrayBlockingQueue",
    "java.util.concurrent.ConcurrentLinkedQueue",
];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Binary { op, lhs, rhs } = &tree.kind else { return };
    let (lhs, rhs) = (strip_parens(lhs), strip_parens(rhs));
    let (call, op, number) = if is_number(rhs, context) {
        (lhs, op.as_str(), context.source(rhs))
    } else if is_number(lhs, context) {
        let swapped = match op.as_str() {
            "<" => ">",
            "<=" => ">=",
            ">" => "<",
            ">=" => "<=",
            other => other,
        };
        (rhs, swapped, context.source(lhs))
    } else {
        return;
    };
    let empty = match (op, number) {
        ("==" | "<=", "0") | ("<", "1") => true,
        ("!=" | ">", "0") | (">=", "1") => false,
        _ => return,
    };
    let K::MethodInvocation { select, args, type_args } = &call.kind else { return };
    let K::MemberSelect { expr: receiver, name: method } = &select.kind else { return };
    let K::Ident(variable) = &receiver.kind else { return };
    if !args.is_empty() || !type_args.is_empty() || !has_is_empty(method, variable, tree, context) {
        return;
    }
    let message = format!("Use isEmpty() instead of comparing {method}() with {number}");
    let diagnostic = Diagnostic::new(Rule::SizeEqualsZero, tree.start, message);
    let negation = if empty { "" } else { "!" };
    diagnostics.push(if context.tokens().has_comment(tree.start..tree.end) {
        diagnostic
    } else {
        diagnostic.with_fix(vec![Edit::new(tree.start..tree.end, format!("{negation}{variable}.isEmpty()"))])
    });
}

fn is_number(tree: &Tree, context: &Context) -> bool {
    matches!(tree.kind, K::Literal(LitKind::Int)) && matches!(context.source(tree), "0" | "1")
}

/// Whether the variable is declared with a type whose `method` (`size` or `length`) has an `isEmpty()` equivalent.
fn has_is_empty(method: &str, variable: &str, tree: &Tree, context: &Context) -> bool {
    let Some(ty) = declared_type(tree, &context.ancestors, variable, context) else { return false };
    let Some(class) = class_name(ty, context) else { return false };
    let ancestors = &context.ancestors;
    match method {
        "size" => COLLECTIONS.iter().any(|c| refers_to(&class, c, ancestors, context)),
        "length" => refers_to(&class, "java.lang.String", ancestors, context),
        _ => false,
    }
}
