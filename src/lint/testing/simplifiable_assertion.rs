//! T037: an assertion that a more specific one says more simply, with a better failure message (IntelliJ
//! SimplifiableAssertion, PMD SimplifiableTestAssertion):
//! - `assertEquals(true, x)` → `assertTrue(x)`, `assertEquals(false, x)` → `assertFalse(x)` (also with
//!   `Boolean.TRUE` and `Boolean.FALSE`), where `x` is known to be a `boolean`: a comparison, a logical operation,
//!   `instanceof`, an `equals` call or a local name declared `boolean`;
//! - `assertEquals(null, x)` → `assertNull(x)`, `assertNotEquals(null, x)` → `assertNotNull(x)`;
//! - `assertTrue(!x)` → `assertFalse(x)` and the reverse;
//! - `assertTrue(x == null)` → `assertNull(x)`, `assertTrue(x != null)` → `assertNotNull(x)`, and the reverse for
//!   `assertFalse`;
//! - `assertTrue(a.equals(b))` → `assertEquals(a, b)`, `assertFalse(a.equals(b))` → `assertNotEquals(a, b)`, with
//!   a constant as the expected value (not `assertNotEquals` for JUnit 3 and TestNG's `AssertJUnit`, which lack it).
//!
//! For JUnit 3, 4 and 5 and TestNG, recognised by the imports or the class the assertion is called on. The message
//! argument is kept in its place.
//!
//! The fix calls the simpler assertion of the same class, importing it like the original if needed. It is safe but
//! for `equals`: `assertEquals(a, b)` passes where `a` is null and `b` too (`a.equals(b)` throws), JUnit 5 compares
//! boxed numbers of different types by value, and JUnit 4 compares arrays by their elements, so that fix is unsafe
//! unless `a` is a string literal. No fix where comments in the call would be lost.

use super::shared::{Framework, Names, compared_values, declared_type, is_constant, method_name, path_with, receiver};
use crate::lint::tree::{Context, is_boolean_literal, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

/// A simpler assertion: the new method, its arguments (sources), and whether the change is safe.
struct Simpler {
    method: &'static str,
    args: Vec<String>,
    safe: bool,
}

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { args, .. } = &tree.kind else { return };
    let Some(name) = method_name(tree) else { return };
    if args.is_empty() || !matches!(name, "assertEquals" | "assertNotEquals" | "assertTrue" | "assertFalse") {
        return;
    }
    let Some(names) = Names::new(context) else { return };
    let Some((framework, _)) = names.assertion(tree, &context.ancestors) else { return };
    let simpler = match name {
        "assertEquals" | "assertNotEquals" => equality(name == "assertEquals", tree, args, framework, context),
        _ => condition(name == "assertTrue", args, framework, context),
    };
    let Some(simpler) = simpler else { return };
    let message = format!("{name}() can be {}()", simpler.method);
    let diagnostic = Diagnostic::new(Rule::SimplifiableAssertion, tree.start, message);
    let class = names.static_method_class(tree, &context.ancestors).unwrap_or_default();
    let new_args: Vec<&str> = simpler.args.iter().map(String::as_str).collect();
    let fix = (!context.tokens().has_comment(tree.start..tree.end))
        .then(|| names.rewrite_static_call(tree, &class, simpler.method, &new_args))
        .flatten();
    diagnostics.push(match fix {
        Some(fix) if simpler.safe => diagnostic.with_fix(fix),
        Some(fix) => diagnostic.with_unsafe_fix(fix),
        None => diagnostic,
    });
}

/// `assertEquals(true, x)`, `assertEquals(null, x)`, `assertNotEquals(null, x)`.
fn equality(equals: bool, call: &Tree, args: &[Tree], framework: Framework, context: &Context) -> Option<Simpler> {
    let (first, second) = compared_values(framework, args)?;
    let (constant, value) = if is_constant(&args[first]) { (first, second) } else { (second, first) };
    let constant_tree = strip_parens(&args[constant]);
    let source = context.source(constant_tree);
    let method = match (equals, &constant_tree.kind) {
        (true, K::Literal(LitKind::Boolean)) | (true, K::MemberSelect { .. }) => {
            let truth = match source {
                "true" | "Boolean.TRUE" => true,
                "false" | "Boolean.FALSE" => false,
                _ => return None,
            };
            if !is_boolean(&args[value], &path_with(context, call), context) {
                return None;
            }
            if truth { "assertTrue" } else { "assertFalse" }
        }
        (true, K::Literal(LitKind::Null)) => "assertNull",
        (false, K::Literal(LitKind::Null)) => "assertNotNull",
        _ => return None,
    };
    let args = args.iter().enumerate().filter(|(i, _)| *i != constant).map(|(_, a)| context.source(a).to_string());
    Some(Simpler { method, args: args.collect(), safe: true })
}

/// `assertTrue(!x)`, `assertTrue(x == null)`, `assertTrue(a.equals(b))` and their `assertFalse` counterparts.
fn condition(is_true: bool, args: &[Tree], framework: Framework, context: &Context) -> Option<Simpler> {
    let index = match args.len() {
        1 => 0,
        2 if framework.message_first() => 1,
        2 => 0,
        _ => return None,
    };
    let condition = strip_parens(&args[index]);
    let source = |tree: &Tree| context.source(tree).to_string();
    let (method, replacement, safe): (&str, Vec<String>, bool) = match &condition.kind {
        K::Unary { op, expr, .. } if op == "!" => {
            (if is_true { "assertFalse" } else { "assertTrue" }, vec![source(strip_parens(expr))], true)
        }
        K::Binary { op, lhs, rhs } if op == "==" || op == "!=" => {
            let value = match (&lhs.kind, &rhs.kind) {
                (_, K::Literal(LitKind::Null)) => lhs,
                (K::Literal(LitKind::Null), _) => rhs,
                _ => return None,
            };
            let null = (op == "==") == is_true;
            (if null { "assertNull" } else { "assertNotNull" }, vec![source(value)], true)
        }
        K::MethodInvocation { args: equals_args, type_args, .. }
            if method_name(condition) == Some("equals") && equals_args.len() == 1 && type_args.is_empty() =>
        {
            let object = receiver(condition)?;
            let no_not_equals = matches!(framework, Framework::JUnit3 | Framework::TestNgJUnit);
            if matches!(&object.kind, K::Ident(name) if name == "super" || name == "this") || !is_true && no_not_equals
            {
                return None;
            }
            let other = &equals_args[0];
            let safe = matches!(strip_parens(object).kind, K::Literal(LitKind::String));
            let (expected, actual) =
                if is_constant(other) && !is_constant(object) { (other, object) } else { (object, other) };
            let pair = if framework.expected_first() { [expected, actual] } else { [actual, expected] };
            let method = if is_true { "assertEquals" } else { "assertNotEquals" };
            (method, pair.iter().map(|t| source(t)).collect(), safe)
        }
        _ => return None,
    };
    let mut new_args: Vec<String> = args.iter().map(|a| context.source(a).to_string()).collect();
    new_args.splice(index..=index, replacement);
    Some(Simpler { method, args: new_args, safe })
}

/// Whether the expression is known to be a `boolean`.
fn is_boolean(tree: &Tree, path: &[&Tree], context: &Context) -> bool {
    let tree = strip_parens(tree);
    match &tree.kind {
        K::Binary { op, .. } => matches!(op.as_str(), "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||"),
        K::Unary { op, .. } => op == "!",
        K::InstanceOf { .. } => true,
        K::MethodInvocation { args, .. } => method_name(tree) == Some("equals") && args.len() == 1,
        K::Ident(name) => declared_type(path, name, context.text) == Some("boolean"),
        _ => is_boolean_literal(tree),
    }
}
