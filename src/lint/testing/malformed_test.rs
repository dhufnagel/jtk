//! T005: a test or lifecycle method that the framework does not run or rejects (IntelliJ JUnitMalformedDeclaration,
//! PMD JUnitJupiterTestNoPrivateModifier):
//! - JUnit 5 (`@Test`, `@ParameterizedTest`, `@RepeatedTest`, `@TestTemplate`, `@TestFactory`, `@BeforeEach`,
//!   `@AfterEach`, `@BeforeAll`, `@AfterAll`): private, static (but `@BeforeAll` and `@AfterAll`), or not void (but
//!   `@TestFactory`). JUnit 5 skips such methods silently.
//! - JUnit 4 (`@Test`, `@Before`, `@After`, `@BeforeClass`, `@AfterClass`): not public, static (but the class
//!   fixtures; see T004 for those that are not static), not void, or with parameters unless the class has a
//!   `@RunWith` runner that may supply them. JUnit 4 fails the class with an initialization error.
//!
//! The annotations are recognised by the imports or the fully qualified name. Parameters of JUnit 5 methods are not
//! reported: extensions resolve them.
//!
//! Unsafe fix for the modifiers: remove `private` or `static`, or make a JUnit 4 method public; the method then
//! runs, and removing `static` may break a static caller. A wrong return type or parameters have no fix.

use super::shared::{Names, keyword, keyword_removal};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, MethodDecl, Modifier, Tree};

/// The JUnit 5 annotations of methods it calls, with whether such a method may be static and must be void.
const JUNIT5: &[(&str, bool, bool)] = &[
    ("org.junit.jupiter.api.Test", false, true),
    ("org.junit.jupiter.api.RepeatedTest", false, true),
    ("org.junit.jupiter.api.TestTemplate", false, true),
    ("org.junit.jupiter.api.TestFactory", false, false),
    ("org.junit.jupiter.params.ParameterizedTest", false, true),
    ("org.junit.jupiter.api.BeforeEach", false, true),
    ("org.junit.jupiter.api.AfterEach", false, true),
    ("org.junit.jupiter.api.BeforeAll", true, true),
    ("org.junit.jupiter.api.AfterAll", true, true),
];

/// The JUnit 4 annotations of methods it calls, with whether such a method must be static (then T004 reports it).
const JUNIT4: &[(&str, bool)] = &[
    ("org.junit.Test", false),
    ("org.junit.Before", false),
    ("org.junit.After", false),
    ("org.junit.BeforeClass", true),
    ("org.junit.AfterClass", true),
];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    if method.mods.annotations.is_empty() || method.ret.is_none() {
        return;
    }
    let Some(names) = Names::new(context) else { return };
    let annotations = names.annotations(&method.mods);
    let text = context.text;
    let report = |message: String, fix: Option<Vec<Edit>>| {
        let diagnostic = Diagnostic::new(Rule::MalformedTest, tree.start, message);
        match fix {
            Some(fix) => diagnostic.with_unsafe_fix(fix),
            None => diagnostic,
        }
    };
    let is_void = method.ret.as_deref().is_some_and(|r| &text[r.start..r.end] == "void");
    if let Some((name, may_be_static, void)) = JUNIT5.iter().find(|(a, ..)| annotations.iter().any(|n| n == a)) {
        let simple = simple_name(name);
        if let Some(private) = keyword(&method.mods, Modifier::Private) {
            let fix = vec![keyword_removal(private, text)];
            diagnostics.push(report(format!("@{simple} method is private"), Some(fix)));
        }
        if let Some(keyword) = keyword(&method.mods, Modifier::Static).filter(|_| !may_be_static) {
            diagnostics.push(report(format!("@{simple} method is static"), Some(vec![keyword_removal(keyword, text)])));
        }
        if *void && !is_void {
            diagnostics.push(report(format!("@{simple} method does not return void"), None));
        }
    } else if let Some((name, class_fixture)) = JUNIT4.iter().find(|(a, _)| annotations.iter().any(|n| n == a)) {
        let simple = simple_name(name);
        if !method.mods.has(Modifier::Public) {
            diagnostics.push(report(format!("@{simple} method is not public"), make_public(method)));
        }
        if let Some(keyword) = keyword(&method.mods, Modifier::Static).filter(|_| !class_fixture) {
            diagnostics.push(report(format!("@{simple} method is static"), Some(vec![keyword_removal(keyword, text)])));
        }
        if !is_void {
            diagnostics.push(report(format!("@{simple} method does not return void"), None));
        }
        if !method.params.is_empty() && !has_runner(context, &names) {
            diagnostics.push(report(format!("@{simple} method has parameters"), None));
        }
    }
}

fn simple_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// The edit that makes a method public: `private` or `protected` replaced, or `public` added before the other
/// modifier keywords or the return type.
fn make_public(method: &MethodDecl) -> Option<Vec<Edit>> {
    let restricting = keyword(&method.mods, Modifier::Private).or_else(|| keyword(&method.mods, Modifier::Protected));
    if let Some(range) = restricting {
        return Some(vec![Edit::new(range, "public")]);
    }
    let position = match method.mods.keywords.first() {
        Some((_, range)) => range.start,
        None if method.type_params.is_empty() && method.return_type_annotations.is_empty() => {
            method.ret.as_ref()?.start
        }
        None => return None,
    };
    Some(vec![Edit::new(position..position, "public ")])
}

/// Whether the class declaring the method has a `@RunWith` runner, which may supply parameters.
fn has_runner(context: &Context, names: &Names) -> bool {
    match context.parent().map(|p| &p.kind) {
        Some(K::Class(class)) => names.has_annotation(&class.mods, &["org.junit.runner.RunWith"]),
        _ => true,
    }
}
