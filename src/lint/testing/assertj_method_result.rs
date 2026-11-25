//! T049: AssertJ asserting the result of a method of the value instead of using the matching assertion (Picnic's
//! AssertJ Refaster rules): `assertThat(list.contains(x)).isTrue()` fails with "expected true", while
//! `assertThat(list).contains(x)` shows the list and the missing element.
//!
//! Narrowed to the method patterns whose replacement is certainly valid for the value's AssertJ assert type, so only
//! where that type is known locally (a string literal, or a name declared in the file with the type) and resolved by
//! the imports:
//! - `String`, `CharSequence`: `isEmpty()`, `contains(s)`, `startsWith(s)`, `endsWith(s)`, `equalsIgnoreCase(s)`,
//!   `matches(regex)`, `equals(o)` with `isTrue()` or `isFalse()`, and `length()` with `isEqualTo(n)`;
//! - `java.util` lists, sets, queues and collections: `isEmpty()`, `contains(e)`, `equals(o)`, and `size()` with
//!   `isEqualTo(n)`;
//! - `java.util` maps: `isEmpty()`, `containsKey(k)`, `containsValue(v)`, `equals(o)`, and `size()`;
//! - `java.util.Optional`: `isPresent()`, `isEmpty()`, `equals(o)`.
//!
//! `contains`, `containsKey` and `containsValue` need type arguments without wildcards (AssertJ's assertions are
//! generic in the element type), and `isEqualTo(n)` an `int` literal. Not covered: files, paths, instants,
//! durations, comparisons and `==` (Picnic's other rules), which need types this linter does not know.
//!
//! Fix: `assertThat(x).contains(y)` (or `doesNotContain(y)` for `isFalse()`), keeping descriptions such as
//! `as(...)`. It is safe: the new assertion checks the same, only where the method would throw on a null value the
//! assertion fails instead. No fix where comments in the call would be lost.

use super::shared::{Names, arguments, local_type, method_name, path_with, receiver};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

/// The kinds of values with assertions for their methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    CharSequence,
    Collection,
    Map,
    Optional,
}

const COLLECTIONS: &[&str] = &[
    "java.util.ArrayDeque",
    "java.util.ArrayList",
    "java.util.Collection",
    "java.util.Deque",
    "java.util.HashSet",
    "java.util.LinkedHashSet",
    "java.util.LinkedList",
    "java.util.List",
    "java.util.NavigableSet",
    "java.util.Queue",
    "java.util.Set",
    "java.util.SortedSet",
    "java.util.TreeSet",
];

const MAPS: &[&str] = &[
    "java.util.HashMap",
    "java.util.LinkedHashMap",
    "java.util.Map",
    "java.util.NavigableMap",
    "java.util.SortedMap",
    "java.util.TreeMap",
];

/// AssertJ's methods that describe the assertion and return it.
const DESCRIPTIONS: &[&str] = &["as", "describedAs", "withFailMessage", "overridingErrorMessage"];

/// The method of the value, its number of arguments, the kinds it applies to, and the assertions replacing
/// `isTrue()` and `isFalse()` on its result.
const BOOLEAN_METHODS: &[(&str, usize, &[Kind], &str, &str)] = &[
    ("isEmpty", 0, &[Kind::CharSequence, Kind::Collection, Kind::Map], "isEmpty", "isNotEmpty"),
    ("isEmpty", 0, &[Kind::Optional], "isEmpty", "isPresent"),
    ("isPresent", 0, &[Kind::Optional], "isPresent", "isEmpty"),
    ("contains", 1, &[Kind::CharSequence, Kind::Collection], "contains", "doesNotContain"),
    ("startsWith", 1, &[Kind::CharSequence], "startsWith", "doesNotStartWith"),
    ("endsWith", 1, &[Kind::CharSequence], "endsWith", "doesNotEndWith"),
    ("equalsIgnoreCase", 1, &[Kind::CharSequence], "isEqualToIgnoringCase", "isNotEqualToIgnoringCase"),
    ("matches", 1, &[Kind::CharSequence], "matches", "doesNotMatch"),
    ("containsKey", 1, &[Kind::Map], "containsKey", "doesNotContainKey"),
    ("containsValue", 1, &[Kind::Map], "containsValue", "doesNotContainValue"),
    ("equals", 1, &[Kind::CharSequence, Kind::Collection, Kind::Map, Kind::Optional], "isEqualTo", "isNotEqualTo"),
];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(assertion) = method_name(tree) else { return };
    if !matches!(assertion, "isTrue" | "isFalse" | "isEqualTo") {
        return;
    }
    // `assertThat(x.m(...))`, possibly with descriptions after it.
    let mut subject = receiver(tree);
    while let Some(call) = subject.filter(|s| method_name(s).is_some_and(|n| DESCRIPTIONS.contains(&n))) {
        subject = receiver(call);
    }
    let Some(assert_that) = subject else { return };
    let [argument] = arguments(assert_that) else { return };
    let argument = strip_parens(argument);
    let Some(method) = method_name(argument) else { return };
    let Some(value) = receiver(argument) else { return };
    let Some(names) = Names::new(context) else { return };
    if !names.is_assertj_assert_that(assert_that, &context.ancestors) {
        return;
    }
    let method_args = arguments(argument);
    let Some((kind, concrete)) = kind(value, &path_with(context, tree), &names, context) else { return };
    let replacement = match (assertion, arguments(tree)) {
        ("isTrue" | "isFalse", []) => BOOLEAN_METHODS
            .iter()
            .find(|(m, count, kinds, ..)| *m == method && *count == method_args.len() && kinds.contains(&kind))
            .filter(|(m, ..)| concrete || !m.starts_with("contains"))
            .map(|(_, _, _, if_true, if_false)| if assertion == "isTrue" { *if_true } else { *if_false }),
        ("isEqualTo", [size]) if matches!(strip_parens(size).kind, K::Literal(LitKind::Int)) => {
            let sized = match kind {
                Kind::CharSequence => method == "length",
                Kind::Collection | Kind::Map => method == "size",
                Kind::Optional => false,
            };
            (sized && method_args.is_empty()).then_some("hasSize")
        }
        _ => None,
    };
    let Some(replacement) = replacement else { return };
    let message = format!("assertThat() of the result of {method}() instead of {replacement}()");
    let diagnostic = Diagnostic::new(Rule::AssertjMethodResult, assert_that.start, message);
    if context.tokens().has_comment(tree.start..tree.end) {
        diagnostics.push(diagnostic);
        return;
    }
    let new_args = match (assertion, arguments(tree)) {
        ("isEqualTo", [size]) => context.source(size).to_string(),
        _ => method_args.iter().map(|a| context.source(a)).collect::<Vec<_>>().join(", "),
    };
    let K::MethodInvocation { select, .. } = &tree.kind else { return };
    let name_start = select.end - assertion.len();
    diagnostics.push(diagnostic.with_fix(vec![
        Edit::new(argument.start..argument.end, context.source(value)),
        Edit::new(name_start..tree.end, format!("{replacement}({new_args})")),
    ]));
}

/// The kind of the value if its type is known locally, and whether its type arguments are concrete (present and
/// without wildcards).
fn kind(value: &Tree, path: &[&Tree], names: &Names, context: &Context) -> Option<(Kind, bool)> {
    let ty = local_type(value, path, context.text)?;
    if ty.ends_with(']') {
        return None;
    }
    let (base, type_args) = match ty.split_once('<') {
        Some((base, rest)) => (base, Some(rest)),
        None => (ty.as_str(), None),
    };
    let resolved = names.resolve_type(base)?;
    let kind = match resolved.as_str() {
        "java.lang.String" | "java.lang.CharSequence" => Kind::CharSequence,
        "java.util.Optional" => Kind::Optional,
        _ if COLLECTIONS.contains(&resolved.as_str()) => Kind::Collection,
        _ if MAPS.contains(&resolved.as_str()) => Kind::Map,
        _ => return None,
    };
    let concrete = type_args.is_some_and(|args| !args.contains('?') && args != ">");
    Some((kind, concrete))
}
