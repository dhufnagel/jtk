//! B280: `throw null`, which throws a `NullPointerException` that does not say why (Error Prone ThrowNull, IntelliJ
//! NullThrown).
//!
//! No fix: which exception to throw instead is the author's choice.

use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

pub fn check(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Throw(expression) = &tree.kind else { return };
    if matches!(strip_parens(expression).kind, K::Literal(LitKind::Null)) {
        diagnostics.push(Diagnostic::new(Rule::ThrowNull, tree.start, "throw null throws a NullPointerException"));
    }
}
