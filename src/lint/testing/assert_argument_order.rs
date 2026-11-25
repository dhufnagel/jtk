//! T032: `assertEquals` with the expected and the actual value swapped (errorprone AssertEqualsArgumentOrderChecker,
//! PMD AssertEqualsArgumentOrder, IntelliJ MisorderedAssertEqualsArguments, NetBeans
//! DN_assertEqualsMismatchedConstantVSReal): the failure message then calls the actual value the expected one.
//!
//! JUnit 3, 4 and 5 (and TestNG's `AssertJUnit`) take the expected value first, TestNG's `Assert` takes it second.
//! Reported are `assertEquals`, `assertNotEquals`, `assertSame`, `assertNotSame` and `assertArrayEquals` where the
//! actual position holds a constant (a literal, a class literal or an upper-case name such as `MAX` or `Color.RED`)
//! and the expected position does not, unless the latter mentions `expected`. Unlike errorprone, other names of the
//! arguments are not considered. The assertion is recognised by the imports or the class it is called on. JUnit 3 and 4 calls of
//! three arguments are only checked when the first is a string literal (a message) or the last a number (a delta).
//!
//! Fix: swap the two arguments. It is safe: the constant has no side effects that could change order. No fix with
//! comments in or between the arguments.

use super::shared::{Names, compared_values, is_constant};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

const METHODS: &[&str] = &["assertEquals", "assertNotEquals", "assertSame", "assertNotSame", "assertArrayEquals"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { args, .. } = &tree.kind else { return };
    if args.len() < 2 || !super::shared::method_name(tree).is_some_and(|n| METHODS.contains(&n)) {
        return;
    }
    let Some(names) = Names::new(context) else { return };
    let Some((framework, name)) = names.assertion(tree, &context.ancestors) else { return };
    let Some((first, second)) = compared_values(framework, args) else { return };
    let (expected, actual) =
        if framework.expected_first() { (&args[first], &args[second]) } else { (&args[second], &args[first]) };
    let named_expected = context.source(expected).to_ascii_lowercase().contains("expected");
    if !is_constant(actual) || is_constant(expected) || named_expected {
        return;
    }
    let message = format!("{name}() with the expected and actual values swapped");
    let diagnostic = Diagnostic::new(Rule::AssertArgumentOrder, tree.start, message);
    let (a, b) = (&args[first], &args[second]);
    // A comment between the arguments would end up describing the other one.
    diagnostics.push(if context.tokens().has_comment(a.start..b.end) {
        diagnostic
    } else {
        diagnostic
            .with_fix(vec![Edit::new(a.start..a.end, context.source(b)), Edit::new(b.start..b.end, context.source(a))])
    });
}
