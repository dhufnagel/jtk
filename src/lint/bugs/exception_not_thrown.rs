//! B219: a statement that only creates an exception, `new IllegalStateException("x");`, where `throw` is missing
//! (Error Prone DeadException, IntelliJ ThrowableNotThrown, NetBeans Created Throwable not thrown).
//!
//! Without types, an exception is a class whose simple name ends in `Exception`, or `Throwable` or one of the errors
//! of `java.lang` resolved through the imports (other names ending in `Error` are often no throwables). Like Error
//! Prone, which skips JUnit test classes, statements in methods annotated `@Test`, in classes extending `TestCase`, in
//! `try` blocks and in lambdas are not reported: there creating the exception may test that its constructor throws.
//!
//! The unsafe fix prepends `throw` (Error Prone's fix for the last statement). It is offered only where the code
//! still compiles: for an unchecked exception of the JDK or one the method declares to throw (or `Exception` or
//! `Throwable`), as the last statement of a method or constructor body or as the branch of an `if` without `else`
//! (where no code after it becomes unreachable).

use super::shared_misc::names_type;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, MethodDecl, Tree};

/// The throwables of `java.lang` whose name does not end in `Exception`: `Throwable` and the errors.
const LANG_ERRORS: &[&str] = &[
    "Throwable",
    "Error",
    "AssertionError",
    "LinkageError",
    "VirtualMachineError",
    "OutOfMemoryError",
    "StackOverflowError",
    "InternalError",
    "UnknownError",
    "ExceptionInInitializerError",
    "NoClassDefFoundError",
    "IncompatibleClassChangeError",
    "AbstractMethodError",
    "NoSuchFieldError",
    "NoSuchMethodError",
    "IllegalAccessError",
    "InstantiationError",
    "UnsatisfiedLinkError",
    "VerifyError",
    "ClassFormatError",
    "BootstrapMethodError",
    "ThreadDeath",
];

/// Unchecked exceptions of the JDK, besides the errors.
const UNCHECKED_EXCEPTIONS: &[&str] = &[
    "java.lang.RuntimeException",
    "java.lang.IllegalArgumentException",
    "java.lang.IllegalStateException",
    "java.lang.NullPointerException",
    "java.lang.UnsupportedOperationException",
    "java.lang.IndexOutOfBoundsException",
    "java.lang.ArrayIndexOutOfBoundsException",
    "java.lang.StringIndexOutOfBoundsException",
    "java.lang.ArithmeticException",
    "java.lang.ClassCastException",
    "java.lang.NumberFormatException",
    "java.lang.ArrayStoreException",
    "java.lang.NegativeArraySizeException",
    "java.lang.SecurityException",
    "java.lang.IllegalMonitorStateException",
    "java.util.NoSuchElementException",
    "java.util.ConcurrentModificationException",
    "java.io.UncheckedIOException",
];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::ExprStmt(expression) = &tree.kind else { return };
    let K::NewClass { ident, .. } = &expression.kind else { return };
    let source = context.source(ident);
    let simple = source.rsplit('.').next().unwrap_or(source).trim();
    let is_lang_error = || LANG_ERRORS.iter().any(|e| names_type(ident, &format!("java.lang.{e}"), context));
    if !simple.ends_with("Exception") && !is_lang_error() {
        return;
    }
    let Some(method) = enclosing_method(tree, context) else { return };
    let Some(method) = method else {
        diagnostics.push(Diagnostic::new(
            Rule::ExceptionNotThrown,
            tree.start,
            format!("{simple} created but not thrown"),
        ));
        return;
    };
    let message = format!("{simple} created but not thrown");
    let diagnostic = Diagnostic::new(Rule::ExceptionNotThrown, tree.start, message);
    let is_error = || LANG_ERRORS[1..].iter().any(|e| names_type(ident, &format!("java.lang.{e}"), context));
    let unchecked = || is_error() || UNCHECKED_EXCEPTIONS.iter().any(|e| names_type(ident, e, context));
    let declared =
        || method.throws.iter().map(|t| context.source(t)).any(|t| [simple, "Exception", "Throwable"].contains(&t));
    diagnostics.push(if (unchecked() || declared()) && is_last_in_branch(tree, method, context) {
        diagnostic.with_unsafe_fix(vec![Edit::new(expression.start..expression.start, "throw ")])
    } else {
        diagnostic
    });
}

/// The method or constructor that encloses the statement (`Some(None)` for an initializer), unless the statement is in
/// a lambda or a `try` block, the method is annotated `@Test` or its class extends `TestCase`.
fn enclosing_method<'t>(statement: &Tree, context: &Context<'t>) -> Option<Option<&'t MethodDecl>> {
    let mut ancestors = context.ancestors.iter().rev();
    let method = loop {
        let ancestor = ancestors.next()?;
        match &ancestor.kind {
            K::Lambda { .. } => return None,
            K::Class(_) | K::NewClass { .. } => return Some(None),
            K::Try { block, .. } if block.start <= statement.start && statement.end <= block.end => return None,
            K::Method(method) => break method,
            _ => {}
        }
    };
    let is_test = method.mods.annotations.iter().any(|a| match &a.kind {
        K::Annotation { ty, .. } => context.source(ty).rsplit('.').next() == Some("Test"),
        _ => false,
    });
    let in_test_case = ancestors.find_map(|a| match &a.kind {
        K::Class(class) => Some(class.extends.as_deref().is_some_and(|e| context.source(e).ends_with("TestCase"))),
        _ => None,
    });
    (!is_test && in_test_case != Some(true)).then_some(Some(&**method))
}

/// Whether the statement is the last of the method's body, or the branch (or the last statement of the branch) of an
/// `if` without `else`: a `throw` there makes no code unreachable.
fn is_last_in_branch(statement: &Tree, method: &MethodDecl, context: &Context) -> bool {
    let is_last_of = |block: &Tree| match &block.kind {
        K::Block { stmts, .. } => stmts.last().is_some_and(|s| std::ptr::eq(s, statement)),
        _ => false,
    };
    let is_branch = |parent: &Tree, child: &Tree| match &parent.kind {
        K::If { then, else_: None, .. } => std::ptr::eq(&**then, child),
        _ => false,
    };
    let (Some(parent), grandparent) = (context.parent(), context.grandparent()) else { return false };
    is_branch(parent, statement)
        || is_last_of(parent)
            && (method.body.as_deref().is_some_and(|body| std::ptr::eq(body, parent))
                || grandparent.is_some_and(|g| is_branch(g, parent)))
}
