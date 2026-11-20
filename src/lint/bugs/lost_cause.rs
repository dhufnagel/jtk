//! B023: `throw new FooException(e.getMessage())` in `catch (FooException e)` (Checkstyle
//! NoCauseGetMessageInExceptionConstructor, PMD PreserveStackTrace): the new exception keeps only the message of the
//! caught one and loses its stack trace; pass the exception as the cause instead.
//!
//! As in Checkstyle, the thrown type is compared by simple name with the names in the parameter of any enclosing
//! `catch`, and `getMessage` must be called directly on that parameter, anywhere in the `throw` statement.

use crate::lint::tree::{Context, for_each_descendant, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree, VarDecl};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Throw(expression) = &tree.kind else { return };
    // Cheap filter before searching the enclosing catch clauses.
    if !context.source(tree).contains("getMessage") {
        return;
    }
    let Some(thrown_type) = thrown_type(expression) else { return };
    let receivers = get_message_receivers(tree);
    if receivers.is_empty() {
        return;
    }
    let in_catch = context.ancestors.iter().any(|ancestor| {
        let K::Try { catches, .. } = &ancestor.kind else { return false };
        catches.iter().any(|c| {
            c.block.start <= tree.start && tree.end <= c.block.end && catches_type(&c.param, thrown_type, &receivers)
        })
    });
    if in_catch {
        diagnostics.push(Diagnostic::new(
            Rule::LostCause,
            tree.start,
            "Avoid using '.getMessage()' in throw statement; pass the exception as the cause",
        ));
    }
}

/// The simple name of the class in `throw new Foo(...)`.
fn thrown_type(expression: &Tree) -> Option<&str> {
    // Not `outer.new Foo()`, whose `new` Checkstyle does not find.
    let K::NewClass { outer: None, ident, .. } = &strip_parens(expression).kind else { return None };
    let mut ty = &**ident;
    loop {
        ty = match &ty.kind {
            K::Parameterized { base, .. } => base,
            K::MemberSelect { name, .. } | K::Ident(name) => return Some(name),
            _ => return None,
        };
    }
}

/// The names `x` in `x.getMessage()` (or `x.getMessage`) in the statement.
fn get_message_receivers(throw: &Tree) -> Vec<&str> {
    let mut receivers = Vec::new();
    for_each_descendant(throw, &mut |tree, _| {
        if let K::MemberSelect { expr, name } = &tree.kind
            && name == "getMessage"
            && let K::Ident(receiver) = &expr.kind
        {
            receivers.push(receiver.as_str());
        }
        true
    });
    receivers
}

/// Whether the catch parameter is one of the receivers and its declaration mentions the type name.
fn catches_type(parameter: &VarDecl, thrown_type: &str, receivers: &[&str]) -> bool {
    if !receivers.contains(&parameter.name.as_str()) {
        return false;
    }
    let mentions =
        |tree: &Tree| matches!(&tree.kind, K::Ident(n) | K::MemberSelect { name: n, .. } if n == thrown_type);
    let mut mentions_type = parameter.name == thrown_type;
    for tree in parameter.mods.annotations.iter().chain(parameter.ty.as_deref()) {
        mentions_type = mentions_type || mentions(tree);
        for_each_descendant(tree, &mut |t, _| {
            mentions_type = mentions_type || mentions(t);
            !mentions_type
        });
    }
    mentions_type
}
