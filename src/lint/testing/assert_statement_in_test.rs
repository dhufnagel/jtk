//! T040: a Java `assert` statement in a test (errorprone UseCorrectAssertInTests, PMD AssertStatementInTest): it
//! checks nothing unless the JVM runs with `-ea`, so the test may pass although the condition is false.
//!
//! Reported in methods annotated as tests or test fixtures of JUnit 4 or 5 or TestNG (`@Test`, `@BeforeEach`, ...,
//! recognised by the imports or the fully qualified name), including lambdas in them.
//!
//! Unsafe fix (the check then runs): `assert x == null` → `assertNull(x)`, `assert x != null` →
//! `assertNotNull(x)`, `assert !x` → `assertFalse(x)`, otherwise `assertTrue(x)`, with the framework's assertions
//! class (imported if needed). A detail message is kept if it is a string literal; with another detail (any object)
//! or comments in the statement there is no fix.

use super::shared::{Framework, Names};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

/// The simple names of the test and fixture annotations.
const TEST_ANNOTATIONS: &[&str] = &[
    "Test",
    "ParameterizedTest",
    "RepeatedTest",
    "TestFactory",
    "TestTemplate",
    "Before",
    "After",
    "BeforeClass",
    "AfterClass",
    "BeforeEach",
    "AfterEach",
    "BeforeAll",
    "AfterAll",
    "BeforeMethod",
    "AfterMethod",
];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Assert { cond, detail } = &tree.kind else { return };
    let Some(method) =
        context.ancestors.iter().rev().take_while(|a| !matches!(a.kind, K::Class(_))).find_map(|a| match &a.kind {
            K::Method(method) => Some(method),
            _ => None,
        })
    else {
        return;
    };
    let Some(names) = Names::new(context) else { return };
    let framework = names.annotations(&method.mods).iter().find_map(|annotation| {
        let simple = annotation.rsplit('.').next()?;
        let framework = Framework::of(annotation)?;
        let test = TEST_ANNOTATIONS.contains(&simple)
            && matches!(framework, Framework::JUnit4 | Framework::JUnit5 | Framework::TestNg);
        test.then_some(framework)
    });
    let Some(framework) = framework else { return };
    let diagnostic = Diagnostic::new(Rule::AssertStatementInTest, tree.start, "assert statement in a test");
    diagnostics.push(match fix(tree, cond, detail.as_deref(), framework, &names, context) {
        Some(fix) => diagnostic.with_unsafe_fix(fix),
        None => diagnostic,
    });
}

fn fix(
    statement: &Tree,
    condition: &Tree,
    detail: Option<&Tree>,
    framework: Framework,
    names: &Names,
    context: &Context,
) -> Option<Vec<Edit>> {
    if context.tokens().has_comment(statement.start..statement.end) {
        return None;
    }
    let message = match detail {
        None => None,
        Some(detail) if matches!(detail.kind, K::Literal(LitKind::String)) => Some(context.source(detail)),
        Some(_) => return None,
    };
    let condition = strip_parens(condition);
    let (method, value) = match &condition.kind {
        K::Binary { op, lhs, rhs } if (op == "==" || op == "!=") && matches!(rhs.kind, K::Literal(LitKind::Null)) => {
            (if op == "==" { "assertNull" } else { "assertNotNull" }, &**lhs)
        }
        K::Unary { op, expr, .. } if op == "!" => ("assertFalse", strip_parens(expr)),
        _ => ("assertTrue", condition),
    };
    let value = context.source(value);
    let args = match message {
        None => value.to_string(),
        Some(message) if framework.message_first() => format!("{message}, {value}"),
        Some(message) => format!("{value}, {message}"),
    };
    let class = framework.assertions_class()?;
    let mut edits = vec![Edit::new(statement.start..statement.end, format!("{method}({args});"))];
    edits.extend(names.static_import(class, method)?);
    Some(edits)
}
