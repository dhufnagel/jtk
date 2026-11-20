//! A010: a method or constructor that declares `throws Exception`, which tells callers nothing about what can go
//! wrong (PMD SignatureDeclareThrowsException with its defaults).
//!
//! As in PMD, test methods and test set-up and tear-down methods (JUnit 3, 4 and 5, TestNG) are skipped, and so are
//! overriding methods: the signature is the overridden method's fault.
//!
//! PMD finds overrides with type information. Here a method overrides if it is annotated with `@Override`, or has the
//! name and arity of a method of `Object` (`clone()`, `finalize()`, ...), or of `Callable.call()`,
//! `PrivilegedExceptionAction.run()` or `AutoCloseable.close()` in a type that names that interface as a supertype:
//! the JDK methods that declare `throws Exception`, whose overrides PMD finds without annotation.
//!
//! Test annotations and JUnit 3's `TestCase` are recognized like PMD does without the project's classpath: only if
//! written fully qualified or imported by a single-type import (not by `import org.junit.*;`). A JUnit 3 test class is
//! a top-level, concrete class extending `TestCase`.

use super::{qualified_name, simple_name, single_type_imports};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassDecl, ClassKind, K, MethodDecl, Modifier, Tree};

/// JUnit 4 and 5 and TestNG annotations of test methods.
const TEST_ANNOTATIONS: &[&str] = &[
    "org.junit.Test",
    "org.testng.annotations.Test",
    "org.junit.jupiter.api.Test",
    "org.junit.jupiter.api.RepeatedTest",
    "org.junit.jupiter.api.TestFactory",
    "org.junit.jupiter.api.TestTemplate",
    "org.junit.jupiter.params.ParameterizedTest",
];

/// JUnit 4 and 5 and TestNG annotations of set-up and tear-down methods.
const CONFIGURATION_ANNOTATIONS: &[&str] = &[
    "org.junit.jupiter.api.BeforeEach",
    "org.junit.jupiter.api.BeforeAll",
    "org.junit.jupiter.api.AfterEach",
    "org.junit.jupiter.api.AfterAll",
    "org.junit.Before",
    "org.junit.BeforeClass",
    "org.junit.After",
    "org.junit.AfterClass",
    "org.testng.annotations.AfterClass",
    "org.testng.annotations.AfterGroups",
    "org.testng.annotations.AfterMethod",
    "org.testng.annotations.AfterSuite",
    "org.testng.annotations.AfterTest",
    "org.testng.annotations.BeforeClass",
    "org.testng.annotations.BeforeGroups",
    "org.testng.annotations.BeforeMethod",
    "org.testng.annotations.BeforeSuite",
    "org.testng.annotations.BeforeTest",
];

/// The methods of `Object` that can be overridden, by name and arity.
const OBJECT_METHODS: &[(&str, usize)] =
    &[("clone", 0), ("finalize", 0), ("equals", 1), ("hashCode", 0), ("toString", 0)];

/// The JDK's interface methods that declare `throws Exception`: interface, method name (without parameters).
const THROWING_INTERFACE_METHODS: &[(&str, &str)] =
    &[("Callable", "call"), ("PrivilegedExceptionAction", "run"), ("AutoCloseable", "close")];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    let Some(first) = method.throws.first() else { return };
    let throws_exception =
        method.throws.iter().any(|t| matches!(qualified_name(t).as_str(), "Exception" | "java.lang.Exception"));
    if !throws_exception || method.ret.is_some() && is_ignored(method, context) {
        return;
    }
    let throws = context.tokens().previous(first.start).map_or(first.start, |t| t.start);
    let message = "A method/constructor should not explicitly throw java.lang.Exception";
    diagnostics.push(Diagnostic::new(Rule::ThrowsException, throws, message));
}

/// Whether the method is a test or test configuration method, or overrides a method.
fn is_ignored(method: &MethodDecl, context: &Context) -> bool {
    let annotations: Vec<String> = method
        .mods
        .annotations
        .iter()
        .filter_map(|a| match &a.kind {
            K::Annotation { ty, .. } => Some(qualified_name(ty)),
            _ => None,
        })
        .collect();
    if annotations.iter().any(|a| a == "Override" || a == "java.lang.Override") {
        return true;
    }
    let unit = context.ancestors[0];
    let is_annotated = |names: &[&str]| annotations.iter().any(|a| names.contains(&resolve(a, unit).as_str()));
    if is_annotated(TEST_ANNOTATIONS) || is_annotated(CONFIGURATION_ANNOTATIONS) {
        return true;
    }
    let owner = context.parent();
    let name = method.name.as_str();
    let is_junit3_method = name == "setUp"
        || name == "tearDown"
        || name.starts_with("test")
            && method.mods.has(Modifier::Public)
            && method.ret.as_deref().is_some_and(|r| matches!(&r.kind, K::Primitive(p) if p == "void"));
    if is_junit3_method && owner.is_some_and(|o| is_junit3_class(o, context)) {
        return true;
    }
    if method.mods.has(Modifier::Static) || method.mods.has(Modifier::Private) {
        return false;
    }
    overrides_jdk_method(method, owner)
}

/// The qualified name of a type name: as written if qualified, else from its single-type import (else unresolved,
/// returned as written).
fn resolve(name: &str, unit: &Tree) -> String {
    if name.contains('.') {
        return name.to_string();
    }
    let suffix = format!(".{name}");
    single_type_imports(unit).find(|imported| imported.ends_with(&suffix)).unwrap_or_else(|| name.to_string())
}

/// Whether the type is a top-level, concrete class extending JUnit 3's `TestCase`.
fn is_junit3_class(owner: &Tree, context: &Context) -> bool {
    let K::Class(class) = &owner.kind else { return false };
    let unit = context.ancestors[0];
    class.kind == ClassKind::Class
        && context.grandparent().is_some_and(|g| std::ptr::eq(g, unit))
        && !class.mods.has(Modifier::Abstract)
        && class.extends.as_deref().is_some_and(|s| resolve(&qualified_name(s), unit) == "junit.framework.TestCase")
}

/// Whether the method overrides one of the JDK's methods that PMD finds by type: a method of `Object`, or of an
/// interface declaring `throws Exception` that its type (a class or anonymous class) implements.
fn overrides_jdk_method(method: &MethodDecl, owner: Option<&Tree>) -> bool {
    let name = method.name.as_str();
    let arity = method.params.len();
    if OBJECT_METHODS.contains(&(name, arity)) {
        return true;
    }
    let supertypes: &[Tree] = match owner.map(|o| &o.kind) {
        Some(K::NewClass { ident, .. }) => std::slice::from_ref(&**ident),
        Some(K::Class(class)) if implements_interfaces(class) => &class.implements,
        _ => &[],
    };
    arity == 0
        && supertypes.iter().any(|t| THROWING_INTERFACE_METHODS.contains(&(simple_name(&qualified_name(t)), name)))
}

/// Whether the type's `implements` clause lists interfaces it implements (an interface's lists those it extends).
fn implements_interfaces(class: &ClassDecl) -> bool {
    matches!(class.kind, ClassKind::Class | ClassKind::Enum | ClassKind::Record)
}
