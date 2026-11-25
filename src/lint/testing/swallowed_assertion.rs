//! T025: an assertion in a `try` block whose `catch` catches `AssertionError`, `Error` or `Throwable` (errorprone
//! AssertionFailureIgnored and TryFailThrowable, IntelliJ MaskedAssertion): when the assertion fails, the catch
//! swallows its error and the test passes. The classic case is `try { f(); fail(); } catch (Throwable t) {}`.
//!
//! Reported are JUnit 3, 4 and 5 and TestNG assertions (`assert...`, `fail`), and AssertJ's `assertThat(...)` and
//! `fail(...)`, recognised by the imports or the class they are called on, directly in the `try` block (not in
//! lambdas or nested classes, which may run elsewhere, nor in a nested `try` that catches the error itself). Not
//! reported if the catch block contains a `throw` or passes the caught error on (stores it, or passes it to a
//! method): it may be rethrown; nor if the `try` is in a loop, which may retry until the assertion passes.
//!
//! No fix: `assertThrows` is the usual replacement, but how the caught error is used varies.

use super::shared::{Names, method_name};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{Catch, K, Tree};

/// The caught types that include `AssertionError`.
const CATCHING_ASSERTIONS: &[&str] = &["java.lang.AssertionError", "java.lang.Error", "java.lang.Throwable"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Try { block, catches, .. } = &tree.kind else { return };
    // A `try` in a loop may retry until the assertion passes.
    let in_loop = context
        .ancestors
        .iter()
        .rev()
        .take_while(|a| !matches!(a.kind, K::Method(_) | K::Lambda { .. } | K::Class(_)))
        .any(|a| matches!(a.kind, K::For { .. } | K::ForEach { .. } | K::While { .. } | K::DoWhile { .. }));
    if catches.is_empty() || in_loop {
        return;
    }
    let Some(names) = Names::new(context) else { return };
    let swallowing =
        |catch: &Catch| catches_assertion_error(catch, &names, context) && !has_throw(&catch.block) && !keeps(catch);
    if !catches.iter().any(swallowing) {
        return;
    }
    let mut enclosing = context.ancestors.clone();
    enclosing.push(tree);
    let mut assertions = Vec::new();
    find_assertions(block, &names, context, &enclosing, &mut assertions);
    for assertion in assertions {
        let message = "Assertion failure caught by the enclosing catch";
        diagnostics.push(Diagnostic::new(Rule::SwallowedAssertion, assertion.start, message));
    }
}

/// Whether the catch clause catches `AssertionError` (or a supertype).
fn catches_assertion_error(catch: &Catch, names: &Names, context: &Context) -> bool {
    let types: Vec<&Tree> = match (&catch.union[..], catch.param.ty.as_deref()) {
        ([], Some(ty)) => vec![ty],
        (union, _) => union.iter().collect(),
    };
    types
        .iter()
        .any(|ty| names.resolve_type(context.source(ty)).is_some_and(|t| CATCHING_ASSERTIONS.contains(&t.as_str())))
}

/// Whether the catch block passes the caught error on (stores it, or passes it to a method): it may be rethrown
/// later. Calling its methods (`e.getMessage()`) does not count.
fn keeps(catch: &Catch) -> bool {
    let name = &catch.param.name;
    let mut found = false;
    crate::lint::tree::for_each_descendant(&catch.block, &mut |child, parent| {
        found |= matches!(&child.kind, K::Ident(n) if n == name) && !matches!(parent.kind, K::MemberSelect { .. });
        !found
    });
    found
}

fn has_throw(tree: &Tree) -> bool {
    let mut found = false;
    crate::lint::tree::for_each_descendant(tree, &mut |child, _| {
        found |= matches!(child.kind, K::Throw(_));
        !found
    });
    found
}

/// Collects the assertions in the code of `tree` that runs where it is: not in lambdas, class bodies, and the
/// blocks of nested `try` statements that catch assertion errors themselves.
fn find_assertions<'t>(
    tree: &'t Tree,
    names: &Names,
    context: &Context,
    enclosing: &[&Tree],
    found: &mut Vec<&'t Tree>,
) {
    tree.for_each_child(&mut |child| match &child.kind {
        K::Lambda { .. } | K::Class(_) => {}
        K::NewClass { args, outer, .. } => {
            for argument in args.iter().chain(outer.as_deref()) {
                find_assertions(argument, names, context, enclosing, found);
            }
        }
        K::Try { resources, block, catches, finally } => {
            let catches_itself = catches.iter().any(|c| catches_assertion_error(c, names, context));
            for part in resources.iter().chain((!catches_itself).then_some(&**block)) {
                find_assertions(part, names, context, enclosing, found);
            }
            for part in catches.iter().map(|c| &*c.block).chain(finally.as_deref()) {
                find_assertions(part, names, context, enclosing, found);
            }
        }
        K::MethodInvocation { .. } if is_assertion(child, names, enclosing) => {
            found.push(child);
        }
        _ => find_assertions(child, names, context, enclosing, found),
    });
}

/// Whether the call is a JUnit or TestNG assertion, AssertJ's `assertThat(...)` or `fail(...)`.
fn is_assertion(call: &Tree, names: &Names, enclosing: &[&Tree]) -> bool {
    if names.assertion(call, enclosing).is_some() || names.is_assertj_assert_that(call, enclosing) {
        return true;
    }
    method_name(call) == Some("fail")
        && names.static_method_class(call, enclosing).is_some_and(|c| c.starts_with("org.assertj.core.api."))
}
