//! T035: an assertion that compares a value with itself (errorprone JUnitAssertSameCheck and SelfAssertion):
//! `assertSame(a, a)` and `assertThat(x).isEqualTo(x)` always pass (or always fail when negated) and test nothing;
//! usually another value was meant.
//!
//! Reported are only the assertions that check identity first and so never call the value's `equals`: JUnit's and
//! TestNG's `assertSame`, `assertNotSame` and `assertArrayEquals`, and AssertJ's `assertThat(x)` followed (possibly
//! after `as(...)` or `describedAs(...)`) by `isEqualTo`, `isNotEqualTo`, `isSameAs` or `isNotSameAs`, recognised by
//! the imports or the class they are called on. `assertEquals(a, a)` is not reported: it calls `a.equals(a)` and so
//! tests the reflexivity of `equals`, which tests of `equals` do on purpose. The two values must be the same names,
//! field accesses or literals (method calls may return different values).
//!
//! No fix: the intended value is unknown.

use super::shared::{Names, arguments, compact, compared_values, method_name, receiver};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

const JUNIT: &[&str] = &["assertSame", "assertNotSame", "assertArrayEquals"];

const ASSERTJ: &[&str] = &["isEqualTo", "isNotEqualTo", "isSameAs", "isNotSameAs"];

/// AssertJ's methods that describe the assertion and return it.
const DESCRIPTIONS: &[&str] = &["as", "describedAs", "withFailMessage", "overridingErrorMessage"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(name) = method_name(tree) else { return };
    let args = arguments(tree);
    let pair = if JUNIT.contains(&name) && args.len() >= 2 {
        let Some(names) = Names::new(context) else { return };
        let Some((framework, _)) = names.assertion(tree, &context.ancestors) else { return };
        let Some((first, second)) = compared_values(framework, args) else { return };
        (&args[first], &args[second])
    } else if ASSERTJ.contains(&name) && args.len() == 1 {
        let Some(assert_that) = assertj_subject(tree) else { return };
        let Some(names) = Names::new(context) else { return };
        if !names.is_assertj_assert_that(assert_that, &context.ancestors) {
            return;
        }
        let [subject] = arguments(assert_that) else { return };
        (subject, &args[0])
    } else {
        return;
    };
    let same = is_pure(pair.0) && compact(context.source(pair.0)) == compact(context.source(pair.1));
    if same {
        diagnostics.push(Diagnostic::new(Rule::SelfAssertion, tree.start, "Assertion compares a value with itself"));
    }
}

/// The `assertThat(...)` call that the AssertJ assertion `call` is chained on, skipping descriptions.
fn assertj_subject(call: &Tree) -> Option<&Tree> {
    let mut current = receiver(call)?;
    while method_name(current).is_some_and(|n| DESCRIPTIONS.contains(&n)) {
        current = receiver(current)?;
    }
    matches!(current.kind, K::MethodInvocation { .. }).then_some(current)
}

/// Whether the expression is a name, a field access on names or a literal: it evaluates to the same value twice.
fn is_pure(tree: &Tree) -> bool {
    match &strip_parens(tree).kind {
        K::Ident(_) | K::Literal(_) => true,
        K::MemberSelect { expr, .. } => is_pure(expr),
        _ => false,
    }
}
