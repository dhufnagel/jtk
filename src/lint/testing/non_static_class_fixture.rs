//! T004: a JUnit 4 `@BeforeClass` or `@AfterClass` method that is not static (errorprone
//! JUnit4ClassAnnotationNonStatic): JUnit 4 fails the whole class with an initialization error. (JUnit 5's
//! `@BeforeAll` may be an instance method with `@TestInstance(PER_CLASS)`, so it is not reported.)
//!
//! Fix: add `static`. It is offered only where it keeps the code compiling: the method has no type parameters, its
//! class is a top-level or static nested class without a superclass, and its body uses neither `this`, `super` nor the name of an
//! instance member of the class. The annotations are recognised by the imports or the fully qualified name.

use super::shared::{Names, is_static_member, mentions};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Modifier, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    if method.mods.has(Modifier::Static) || method.mods.annotations.is_empty() {
        return;
    }
    let Some(names) = Names::new(context) else { return };
    let annotations = names.annotations(&method.mods);
    let annotation = if annotations.iter().any(|a| a == "org.junit.BeforeClass") {
        "@BeforeClass"
    } else if annotations.iter().any(|a| a == "org.junit.AfterClass") {
        "@AfterClass"
    } else {
        return;
    };
    let diagnostic =
        Diagnostic::new(Rule::NonStaticClassFixture, tree.start, format!("{annotation} method is not static"));
    let fix_position = match (&method.ret, method.type_params.is_empty(), method.return_type_annotations.is_empty()) {
        (Some(ret), true, true) if can_be_static(tree, context) => Some(ret.start),
        _ => None,
    };
    diagnostics.push(match fix_position {
        Some(position) => diagnostic.with_fix(vec![Edit::new(position..position, "static ")]),
        None => diagnostic,
    });
}

/// Whether the method can be made static: its class may have static methods and the body uses no instance member.
fn can_be_static(method: &Tree, context: &Context) -> bool {
    let Some(K::Class(class)) = context.parent().map(|p| &p.kind) else { return false };
    let top_level = matches!(context.grandparent().map(|g| &g.kind), Some(K::CompilationUnit { .. }));
    let static_nested =
        class.mods.has(Modifier::Static) && context.grandparent().is_some_and(|g| matches!(g.kind, K::Class(_)));
    // A superclass may have instance members the body uses.
    if !top_level && !static_nested || class.extends.is_some() {
        return false;
    }
    let K::Method(declaration) = &method.kind else { return false };
    let Some(body) = &declaration.body else { return false };
    let instance_names = class.members.iter().filter(|m| !is_static_member(m)).flat_map(|m| match &m.kind {
        K::Vars(vars) => vars.iter().map(|v| v.name.as_str()).collect(),
        K::Method(method) => vec![method.name.as_str()],
        _ => Vec::new(),
    });
    let mut used = ["this", "super"].into_iter().chain(instance_names);
    !used.any(|name| mentions(body, name))
}
