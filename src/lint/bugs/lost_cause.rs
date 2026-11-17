//! B023: `throw new FooException(e.getMessage())` in `catch (FooException e)` (Checkstyle
//! NoCauseGetMessageInExceptionConstructor, PMD PreserveStackTrace): the new exception keeps only the message of the
//! caught one and loses its stack trace; pass the exception as the cause instead.
//!
//! As in Checkstyle, the thrown type is compared by simple name with the names in the parameter of any enclosing
//! `catch`, and `getMessage` must be called directly on that parameter, anywhere in the `throw` statement.

use tree_sitter::Node;

use crate::lint::syntax::{for_each_descendant, is_comment, source, strip_parentheses};
use crate::lint::{Diagnostic, Rule};

pub fn check(throw: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    // Cheap filter before searching the enclosing catch clauses.
    if !source(throw, text).contains("getMessage") {
        return;
    }
    let Some(thrown_type) = thrown_type(throw, text) else { return };
    let receivers = get_message_receivers(throw, text);
    if receivers.is_empty() {
        return;
    }
    let mut ancestor = throw.parent();
    while let Some(node) = ancestor {
        if node.kind() == "catch_clause" && catches(&node, text, thrown_type, &receivers) {
            let keyword = throw.child(0).unwrap_or(*throw);
            diagnostics.push(Diagnostic::new(
                Rule::LostCause,
                keyword.start_byte(),
                "Avoid using '.getMessage()' in throw statement; pass the exception as the cause",
            ));
            return;
        }
        ancestor = node.parent();
    }
}

/// The simple name of the class in `throw new Foo(...)`.
fn thrown_type<'a>(throw: &Node, text: &'a str) -> Option<&'a str> {
    let expression = strip_parentheses(&throw.named_children(&mut throw.walk()).find(|c| !is_comment(c))?);
    // Not `outer.new Foo()`, whose `new` Checkstyle does not find.
    if expression.kind() != "object_creation_expression" || expression.child(0).is_none_or(|c| c.kind() != "new") {
        return None;
    }
    let mut ty = expression.child_by_field_name("type")?;
    loop {
        ty = match ty.kind() {
            "generic_type" => ty.named_child(0)?,
            "scoped_type_identifier" => {
                ty.named_children(&mut ty.walk()).filter(|c| c.kind() == "type_identifier").last()?
            }
            "type_identifier" => return Some(source(&ty, text)),
            _ => return None,
        };
    }
}

/// The names `x` in `x.getMessage()` in the statement.
fn get_message_receivers<'a>(throw: &Node, text: &'a str) -> Vec<&'a str> {
    let mut receivers = Vec::new();
    for_each_descendant(throw, &[], |node| {
        let (object, name) = match node.kind() {
            "method_invocation" => (node.child_by_field_name("object"), node.child_by_field_name("name")),
            "field_access" => (node.child_by_field_name("object"), node.child_by_field_name("field")),
            _ => return,
        };
        if let (Some(object), Some(name)) = (object, name)
            && object.kind() == "identifier"
            && source(&name, text) == "getMessage"
        {
            receivers.push(source(&object, text));
        }
    });
    receivers
}

/// Whether the catch parameter is one of the receivers and its declaration mentions the type name.
fn catches(catch: &Node, text: &str, thrown_type: &str, receivers: &[&str]) -> bool {
    let Some(parameter) = catch.named_children(&mut catch.walk()).find(|c| c.kind() == "catch_formal_parameter") else {
        return false;
    };
    let named = parameter.child_by_field_name("name").is_some_and(|n| receivers.contains(&source(&n, text)));
    let mut mentions_type = false;
    for_each_descendant(&parameter, &[], |n| {
        mentions_type |= matches!(n.kind(), "identifier" | "type_identifier") && source(n, text) == thrown_type;
    });
    named && mentions_type
}
