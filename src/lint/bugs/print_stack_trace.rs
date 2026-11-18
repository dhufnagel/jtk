//! B022: output meant for debugging that bypasses logging: `e.printStackTrace()` and `System.out.print…(…)` or
//! `System.err.print…(…)` (PMD AvoidPrintStackTrace, SystemPrintln).
//!
//! PMD checks with type information that `printStackTrace()` is `Throwable`'s method and that `System` is
//! `java.lang.System`; without types, any `printStackTrace()` without arguments counts, and `System` is taken by name.

use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { select, args, .. } = &tree.kind else { return };
    let (name, object) = match &select.kind {
        K::Ident(name) => (name, None),
        K::MemberSelect { expr, name } => (name, Some(&**expr)),
        _ => return,
    };
    let message = if name == "printStackTrace" && args.is_empty() {
        "Avoid printStackTrace(); use a logger call instead."
    } else if name.starts_with("print") && object.is_some_and(|o| is_system_out(o, context.text)) {
        "Usage of System.out/err"
    } else {
        return;
    };
    diagnostics.push(Diagnostic::new(Rule::PrintStackTrace, tree.start, message));
}

/// Whether the expression is `System.out` or `System.err`, possibly qualified as `java.lang.System`.
fn is_system_out(object: &Tree, text: &str) -> bool {
    let K::MemberSelect { expr: class, name: field } = &object.kind else { return false };
    let class: String = text[class.start..class.end].split_whitespace().collect();
    matches!(field.as_str(), "out" | "err") && matches!(class.as_str(), "System" | "java.lang.System")
}
