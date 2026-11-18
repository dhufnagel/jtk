//! S011: `super();` without arguments, which the compiler adds anyway (Checkstyle
//! AvoidNoArgumentSuperConstructorCall). Qualified calls (`outer.super()`) and calls with type arguments are allowed.
//!
//! The fix removes the call, but only as the first statement: since Java 25, statements may precede it, and they
//! would then run after the implicit call.

use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

use super::line_removal;

pub fn check(method: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(constructor) = &method.kind else { return };
    if constructor.ret.is_some() || constructor.compact_ctor {
        return;
    }
    let Some(K::Block { stmts, .. }) = constructor.body.as_deref().map(|b| &b.kind) else { return };
    for (index, call) in stmts.iter().filter(|s| !matches!(s.kind, K::Empty)).enumerate() {
        if !is_no_argument_super_call(call) {
            continue;
        }
        let message = "Unnecessary call to the superclass constructor without arguments";
        let mut diagnostic = Diagnostic::new(Rule::SuperConstructorCall, call.start, message);
        if index == 0 && !context.tokens().has_comment(call.start..call.end) {
            diagnostic = diagnostic.with_fix(vec![line_removal(call.start..call.end, context.text)]);
        }
        diagnostics.push(diagnostic);
    }
}

fn is_no_argument_super_call(statement: &Tree) -> bool {
    let K::ExprStmt(call) = &statement.kind else { return false };
    let K::MethodInvocation { select, type_args, args } = &call.kind else { return false };
    matches!(&select.kind, K::Ident(name) if name == "super") && type_args.is_empty() && args.is_empty()
}
