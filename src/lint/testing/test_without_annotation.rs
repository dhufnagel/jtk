//! T001: a method named like a JUnit 3 test (`testFoo`) without `@Test` in a JUnit 4 or 5 test class (errorprone
//! JUnit4TestNotRun, IntelliJ JUnit3StyleTestMethodInJUnit4Class, PMD DetachedTestCase and
//! UnitTestShouldUseTestAnnotation): JUnit 4 and 5 run only annotated methods, so it never runs.
//!
//! Narrower than the originals: the class must have a method annotated with JUnit 4's or 5's `@Test` (recognised by
//! the imports or the fully qualified name) and must not extend JUnit 3's `TestCase` (or a class named `...TestCase`);
//! the method must be named `test`
//! followed by an upper-case letter, a digit or `_`, have no annotations and no parameters, return void, not be
//! static, abstract or private (public for JUnit 4), and not be called or referenced anywhere in the file (then it is
//! a helper). Methods only containing assertions (errorprone's other signal) are not reported.
//!
//! Unsafe fix: add the class's `@Test` annotation (as written on its other test methods), which makes the method
//! run.

use super::shared::Names;
use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Modifier, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    let Some(names) = Names::new(context) else { return };
    let extends_test_case = class.extends.as_deref().is_some_and(|e| {
        context.source(e).ends_with("TestCase")
            || names.resolve_type(context.source(e)).is_some_and(|t| t == "junit.framework.TestCase")
    });
    if extends_test_case {
        return;
    }
    // The `@Test` annotation of the class's test methods, and its version.
    let test_annotation = class.members.iter().find_map(|member| {
        let K::Method(method) = &member.kind else { return None };
        method.mods.annotations.iter().find_map(|annotation| {
            let name = names.annotation(annotation)?;
            let K::Annotation { ty, .. } = &annotation.kind else { return None };
            match name.as_str() {
                "org.junit.Test" => Some((context.source(ty), true)),
                "org.junit.jupiter.api.Test" => Some((context.source(ty), false)),
                _ => None,
            }
        })
    });
    let Some((annotation, junit4)) = test_annotation else { return };
    let unit = context.ancestors.first().copied().unwrap_or(tree);
    for member in &class.members {
        let K::Method(method) = &member.kind else { continue };
        let visible = if junit4 { method.mods.has(Modifier::Public) } else { !method.mods.has(Modifier::Private) };
        let looks_like_test = method
            .name
            .strip_prefix("test")
            .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'));
        let is_void = method.ret.as_deref().is_some_and(|r| context.source(r) == "void");
        if !looks_like_test
            || !visible
            || !is_void
            || !method.params.is_empty()
            || !method.mods.annotations.is_empty()
            || method.mods.has(Modifier::Static)
            || method.mods.has(Modifier::Abstract)
            || method.body.is_none()
            || is_used(unit, &method.name)
        {
            continue;
        }
        let fix = vec![Edit::new(member.start..member.start, format!("@{annotation} "))];
        diagnostics.push(
            Diagnostic::new(Rule::TestWithoutAnnotation, member.start, "Test method without @Test is not run")
                .with_unsafe_fix(fix),
        );
    }
}

/// Whether a method of that name is called or referenced in the file.
fn is_used(unit: &Tree, name: &str) -> bool {
    let mut used = false;
    for_each_descendant(unit, &mut |tree, _| {
        used |= match &tree.kind {
            K::MethodInvocation { select, .. } => match &select.kind {
                K::Ident(n) | K::MemberSelect { name: n, .. } => n == name,
                _ => false,
            },
            K::MemberRef { name: Some(n), .. } => n == name,
            _ => false,
        };
        !used
    });
    used
}
