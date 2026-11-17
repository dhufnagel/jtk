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

use tree_sitter::Node;

use super::{has_keyword, qualified_name, root};
use crate::lint::syntax::{modifiers, named_children, source};
use crate::lint::{Diagnostic, Rule};

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

pub fn check(declaration: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(throws) = named_children(declaration).into_iter().find(|c| c.kind() == "throws") else { return };
    let throws_exception = named_children(&throws)
        .iter()
        .any(|t| matches!(qualified_name(t, text).as_str(), "Exception" | "java.lang.Exception"));
    if !throws_exception || declaration.kind() == "method_declaration" && is_ignored(declaration, text) {
        return;
    }
    let message = "A method/constructor should not explicitly throw java.lang.Exception";
    diagnostics.push(Diagnostic::new(Rule::ThrowsException, throws.start_byte(), message));
}

/// Whether the method is a test or test configuration method, or overrides a method.
fn is_ignored(method: &Node, text: &str) -> bool {
    let annotations = annotation_names(method, text);
    if annotations.iter().any(|a| a == "Override" || a == "java.lang.Override") {
        return true;
    }
    // Only the rare methods that would be reported get here: the expensive `Node::parent` is fine.
    let Some(body) = method.parent() else { return false };
    let Some(owner) = body.parent() else { return false };
    let program = root(&owner);
    let is_annotated =
        |names: &[&str]| annotations.iter().any(|a| names.contains(&resolve(a, &program, text).as_str()));
    if is_annotated(TEST_ANNOTATIONS) || is_annotated(CONFIGURATION_ANNOTATIONS) {
        return true;
    }
    let name = method.child_by_field_name("name").map_or("", |n| source(&n, text));
    let is_junit3_method = name == "setUp"
        || name == "tearDown"
        || name.starts_with("test")
            && has_keyword(method, text, "public")
            && method.child_by_field_name("type").is_some_and(|t| t.kind() == "void_type");
    if is_junit3_method && is_junit3_class(&owner, &program, text) {
        return true;
    }
    overrides_jdk_method(method, &owner, text)
}

/// The names of the method's annotations, as written.
fn annotation_names(method: &Node, text: &str) -> Vec<String> {
    let Some(modifiers) = modifiers(method) else { return Vec::new() };
    named_children(&modifiers)
        .iter()
        .filter(|m| matches!(m.kind(), "annotation" | "marker_annotation"))
        .filter_map(|a| a.child_by_field_name("name"))
        .map(|n| qualified_name(&n, text))
        .collect()
}

/// The qualified name of a type name: as written if qualified, else from its single-type import (else unresolved,
/// returned as written).
fn resolve(name: &str, program: &Node, text: &str) -> String {
    if name.contains('.') {
        return name.to_string();
    }
    let suffix = format!(".{name}");
    named_children(program)
        .iter()
        .filter(|c| c.kind() == "import_declaration" && !named_children(c).iter().any(|n| n.kind() == "asterisk"))
        .filter_map(|i| named_children(i).into_iter().find(|n| matches!(n.kind(), "scoped_identifier" | "identifier")))
        .map(|n| qualified_name(&n, text))
        .find(|imported| imported.ends_with(&suffix))
        .unwrap_or_else(|| name.to_string())
}

/// Whether the type is a top-level, concrete class extending JUnit 3's `TestCase`.
fn is_junit3_class(owner: &Node, program: &Node, text: &str) -> bool {
    owner.kind() == "class_declaration"
        && owner.parent().is_some_and(|p| p.kind() == "program")
        && !has_keyword(owner, text, "abstract")
        && owner.child_by_field_name("superclass").is_some_and(|s| {
            named_children(&s)
                .iter()
                .any(|t| resolve(&qualified_name(t, text), program, text) == "junit.framework.TestCase")
        })
}

/// Whether the method overrides one of the JDK's methods that PMD finds by type: a method of `Object`, or of an
/// interface declaring `throws Exception` that its type (a class or anonymous class) implements.
fn overrides_jdk_method(method: &Node, owner: &Node, text: &str) -> bool {
    if has_keyword(method, text, "static") || has_keyword(method, text, "private") {
        return false;
    }
    let name = method.child_by_field_name("name").map_or("", |n| source(&n, text));
    let arity = method.child_by_field_name("parameters").map_or(0, |p| {
        named_children(&p).iter().filter(|c| matches!(c.kind(), "formal_parameter" | "spread_parameter")).count()
    });
    if OBJECT_METHODS.contains(&(name, arity)) {
        return true;
    }
    let supertypes: Vec<Node> = match owner.kind() {
        "object_creation_expression" => owner.child_by_field_name("type").into_iter().collect(),
        "class_declaration" | "enum_declaration" | "record_declaration" => owner
            .child_by_field_name("interfaces")
            .into_iter()
            .flat_map(|i| named_children(&i))
            .flat_map(|list| named_children(&list))
            .collect(),
        _ => Vec::new(),
    };
    arity == 0
        && supertypes.iter().any(|t| {
            let supertype = qualified_name(t, text);
            let simple = supertype.rsplit('.').next().unwrap_or_default();
            THROWING_INTERFACE_METHODS.contains(&(simple, name))
        })
}
