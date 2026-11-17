//! S011: `super();` without arguments, which the compiler adds anyway (Checkstyle
//! AvoidNoArgumentSuperConstructorCall). Qualified calls (`outer.super()`) and calls with type arguments are allowed.
//!
//! The fix removes the call, but only as the first statement: since Java 25, statements may precede it, and they
//! would then run after the implicit call.

use tree_sitter::Node;

use super::{contains_comment, line_removal_range};
use crate::lint::syntax::{is_comment, named_children};
use crate::lint::{Diagnostic, Rule};

pub fn check(body: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let statements: Vec<Node> = named_children(body).into_iter().filter(|s| !is_comment(s)).collect();
    for (index, call) in statements.iter().enumerate() {
        if call.kind() != "explicit_constructor_invocation" || !is_no_argument_super_call(call) {
            continue;
        }
        let message = "Unnecessary call to the superclass constructor without arguments";
        let mut diagnostic = Diagnostic::new(Rule::SuperConstructorCall, call.start_byte(), message);
        if index == 0 && !contains_comment(call) {
            diagnostic = diagnostic.with_fix(vec![line_removal_range(call, text)]);
        }
        diagnostics.push(diagnostic);
    }
}

fn is_no_argument_super_call(call: &Node) -> bool {
    call.child_by_field_name("constructor").is_some_and(|c| c.kind() == "super")
        && call.child_by_field_name("object").is_none()
        && call.child_by_field_name("type_arguments").is_none()
        && call.child_by_field_name("arguments").is_some_and(|a| named_children(&a).iter().all(is_comment))
}
