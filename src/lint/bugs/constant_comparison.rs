//! B087: a comparison that is always true or always false because of the range of a size or index, as in
//! `list.size() >= 0` or `s.indexOf("a") >= -1` (IntelliJ PointlessIndexOfComparison, Error Prone
//! SizeGreaterThanOrEqualsZero). A size is never negative and `indexOf` never returns less than -1; such a check was
//! probably meant as `> 0` or `>= 0`.
//!
//! No fix: what was meant is unclear.
//!
//! Without types, a user's `size()` or `indexOf` may return anything, so like Error Prone only the methods of known
//! JDK types count, and only on receivers whose type is known in the file: string literals and variables declared
//! with such a type (`size()` of collections, maps and `ByteArrayOutputStream`; `length()` of strings and
//! builders; `indexOf` and `lastIndexOf` of strings, builders and lists; `length` of arrays). Calls on other
//! expressions (`getList().size()`) are not reported, nor protocol buffer counts. The comparison is with an integer
//! literal on either side.

use super::shared_arithmetic::{integer_literal, is_comparison, method_name, mirrored, variable_type};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

const COLLECTIONS: &[&str] = &[
    "Collection",
    "List",
    "Set",
    "Map",
    "Queue",
    "Deque",
    "SortedSet",
    "SortedMap",
    "NavigableSet",
    "NavigableMap",
    "ArrayList",
    "LinkedList",
    "HashSet",
    "LinkedHashSet",
    "TreeSet",
    "HashMap",
    "LinkedHashMap",
    "TreeMap",
    "ArrayDeque",
    "PriorityQueue",
    "Vector",
    "Stack",
    "Hashtable",
    "EnumSet",
    "EnumMap",
    "IdentityHashMap",
    "WeakHashMap",
    "ByteArrayOutputStream",
    "java.io.ByteArrayOutputStream",
];
const STRINGS: &[&str] = &["String", "StringBuilder", "StringBuffer", "CharSequence"];
const INDEXED: &[&str] =
    &["String", "StringBuilder", "StringBuffer", "List", "ArrayList", "LinkedList", "Vector", "Stack"];

pub fn check<'t>(tree: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let K::Binary { op, lhs, rhs } = &tree.kind else { return };
    if !is_comparison(op) {
        return;
    }
    let text = context.text;
    let (bounded, op, constant) = match (integer_literal(lhs, text), integer_literal(rhs, text)) {
        (None, Some(c)) => (lhs, op.as_str(), c),
        (Some(c), None) => (rhs, mirrored(op), c),
        _ => return,
    };
    let Some((what, minimum)) = lower_bound(bounded, tree, context) else { return };
    let c = constant.wide();
    // The comparison with the operand on the left, against its least value.
    let result = match op {
        ">=" if c <= minimum => true,
        ">" if c < minimum => true,
        "!=" if c < minimum => true,
        "<" if c <= minimum => false,
        "<=" if c < minimum => false,
        "==" if c < minimum => false,
        _ => return,
    };
    let message = format!("{what} is never less than {minimum}, so the comparison is always {result}");
    diagnostics.push(Diagnostic::new(Rule::ConstantComparison, tree.start, message));
}

/// What the expression is and its least value, if it is a size, length or index of a JDK type known in the file.
fn lower_bound<'t>(expression: &'t Tree, tree: &'t Tree, context: &Context<'t>) -> Option<(&'static str, i64)> {
    let expression = strip_parens(expression);
    let receiver_type = |receiver: &'t Tree| -> Option<&'t str> {
        match &strip_parens(receiver).kind {
            K::Literal(LitKind::String) => Some("String"),
            _ => variable_type(context, tree, strip_parens(receiver)),
        }
    };
    match &expression.kind {
        K::MemberSelect { expr, name } if name == "length" => {
            receiver_type(expr).filter(|t| t.ends_with(']')).map(|_| ("An array length", 0))
        }
        K::MethodInvocation { select, args, .. } => {
            let K::MemberSelect { expr: receiver, .. } = &select.kind else { return None };
            let ty = receiver_type(receiver)?;
            match (method_name(select)?, args.len()) {
                ("size", 0) if COLLECTIONS.contains(&ty) => Some(("size()", 0)),
                ("length", 0) if STRINGS.contains(&ty) => Some(("length()", 0)),
                ("indexOf" | "lastIndexOf", 1 | 2) if INDEXED.contains(&ty) => Some(("indexOf()", -1)),
                _ => None,
            }
        }
        _ => None,
    }
}
