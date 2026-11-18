//! S021: a block lambda on one line whose only statement is an expression or returns one, `x -> { return e; }`
//! instead of `x -> e` (Checkstyle ExpressionOverBlockLambda).
//!
//! The fix replaces `{ return e; }` by `e`. It is skipped for expression statements (`() -> { run(); }`): as an
//! expression lambda, `run()` would also be value-compatible, which can select another overload
//! (`submit(Callable)` instead of `submit(Runnable)`). For the same reason it is skipped for lambdas with implicitly
//! typed parameters returning a method call, assignment or object creation, where it can make a call ambiguous.

use crate::ast::{K, Tree, VarDecl};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(lambda: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Lambda { params, body } = &lambda.kind else { return };
    let K::Block { stmts, .. } = &body.kind else { return };
    let Some(arrow) = context.tokens().previous(body.start) else { return };
    if context.text[arrow.start..body.end].contains('\n') {
        return;
    }
    let [statement] = &stmts[..] else { return };
    let expression = match &statement.kind {
        K::ExprStmt(_) => None,
        K::Return(Some(expression)) => Some(expression),
        _ => return,
    };
    let message = "Use an expression lambda instead of a single-line block lambda";
    let mut diagnostic = Diagnostic::new(Rule::ExpressionLambda, arrow.start, message);
    let fixable = expression.filter(|e| {
        !context.tokens().has_comment(body.start..body.end)
            && !(is_implicitly_typed(params, context) && is_statement_expression(e))
    });
    if let Some(expression) = fixable {
        diagnostic = diagnostic.with_fix(vec![Edit::new(body.start..body.end, context.source(expression))]);
    }
    diagnostics.push(diagnostic);
}

fn is_implicitly_typed(params: &[VarDecl], context: &Context) -> bool {
    params.iter().any(|p| p.ty.as_deref().is_none_or(|t| context.source(t) == "var"))
}

/// An expression that may also be a statement, and so makes an expression lambda void-compatible.
fn is_statement_expression(expression: &Tree) -> bool {
    match &expression.kind {
        K::MethodInvocation { .. } | K::NewClass { .. } | K::Assign { .. } => true,
        K::Unary { op, .. } => matches!(op.as_str(), "++" | "--"),
        _ => false,
    }
}
