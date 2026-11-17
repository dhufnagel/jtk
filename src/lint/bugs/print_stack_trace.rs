//! B022: output meant for debugging that bypasses logging: `e.printStackTrace()` and `System.out.print…(…)` or
//! `System.err.print…(…)` (PMD AvoidPrintStackTrace, SystemPrintln).
//!
//! PMD checks with type information that `printStackTrace()` is `Throwable`'s method and that `System` is
//! `java.lang.System`; without types, any `printStackTrace()` without arguments counts, and `System` is taken by name.

use tree_sitter::Node;

use crate::lint::syntax::{named_children, source};
use crate::lint::{Diagnostic, Rule};

pub fn check(call: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(name) = call.child_by_field_name("name") else { return };
    let name = source(&name, text);
    let message = if name == "printStackTrace" && has_no_arguments(call) {
        "Avoid printStackTrace(); use a logger call instead."
    } else if name.starts_with("print") && call.child_by_field_name("object").is_some_and(|o| is_system_out(&o, text)) {
        "Usage of System.out/err"
    } else {
        return;
    };
    diagnostics.push(Diagnostic::new(Rule::PrintStackTrace, call.start_byte(), message));
}

fn has_no_arguments(call: &Node) -> bool {
    call.child_by_field_name("arguments").is_some_and(|a| named_children(&a).iter().all(|c| c.is_extra()))
}

/// Whether the expression is `System.out` or `System.err`, possibly qualified as `java.lang.System`.
fn is_system_out(object: &Node, text: &str) -> bool {
    if object.kind() != "field_access" {
        return false;
    }
    let field = object.child_by_field_name("field").map(|f| source(&f, text));
    let class = object.child_by_field_name("object").map(|o| source(&o, text).split_whitespace().collect::<String>());
    matches!(field, Some("out" | "err")) && matches!(class.as_deref(), Some("System" | "java.lang.System"))
}
