//! T062: Mockito's `verify(mock)`, `when(call)` or BDDMockito's `given(call)` as a statement of its own
//! (errorprone MockitoUsage): nothing is verified or stubbed, and Mockito reports the unfinished verification or
//! stubbing only later, at another call, if at all. `verify(mock.f())` is the common slip for `verify(mock).f()`.
//!
//! No fix: what was meant (the verified call, the stubbed answer) is unknown. The methods are recognised by the
//! static imports or the class they are called on (`Mockito.verify`), never by their names alone.

use super::shared::{Framework, Names};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(K::ExprStmt(expression)) = context.parent().map(|p| &p.kind) else { return };
    if !std::ptr::eq(&**expression, tree) {
        return;
    }
    let Some(names) = Names::new(context) else { return };
    let Some((Framework::Mockito, name)) = names.framework_call(tree, &context.ancestors) else { return };
    let message = match name {
        "verify" => "verify() without the verified call",
        "when" | "given" => "Stubbing without a return value or answer",
        _ => return,
    };
    diagnostics.push(Diagnostic::new(Rule::IncompleteMockitoCall, tree.start, message));
}
