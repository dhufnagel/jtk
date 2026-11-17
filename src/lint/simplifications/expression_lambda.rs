//! S021: a block lambda on one line whose only statement is an expression or returns one, `x -> { return e; }`
//! instead of `x -> e` (Checkstyle ExpressionOverBlockLambda).
//!
//! The fix replaces `{ return e; }` by `e`. It is skipped for expression statements (`() -> { run(); }`): as an
//! expression lambda, `run()` would also be value-compatible, which can select another overload
//! (`submit(Callable)` instead of `submit(Runnable)`). For the same reason it is skipped for lambdas with implicitly
//! typed parameters returning a method call, assignment or object creation, where it can make a call ambiguous.

use tree_sitter::Node;

use super::contains_comment;
use crate::lint::syntax::{is_comment, named_children, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(lambda: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(body) = lambda.child_by_field_name("body").filter(|b| b.kind() == "block") else { return };
    let Some(arrow) = lambda.children(&mut lambda.walk()).find(|c| c.kind() == "->") else { return };
    if arrow.start_position().row != body.end_position().row {
        return;
    }
    let statements: Vec<Node> = body.children(&mut body.walk()).filter(|c| c.is_named() && !is_comment(c)).collect();
    let empty_statements = body.children(&mut body.walk()).filter(|c| c.kind() == ";").count();
    let [statement] = statements[..] else { return };
    if empty_statements > 0 {
        return;
    }
    let expression = match statement.kind() {
        "expression_statement" => None,
        "return_statement" => match named_children(&statement).into_iter().find(|c| !is_comment(c)) {
            Some(expression) => Some(expression),
            None => return,
        },
        _ => return,
    };
    let message = "Use an expression lambda instead of a single-line block lambda";
    let mut diagnostic = Diagnostic::new(Rule::ExpressionLambda, arrow.start_byte(), message);
    let fixable = expression
        .filter(|e| !contains_comment(&body) && !(is_implicitly_typed(lambda, text) && is_statement_expression(e)));
    if let Some(expression) = fixable {
        diagnostic = diagnostic.with_fix(vec![Edit::new(body.byte_range(), source(&expression, text))]);
    }
    diagnostics.push(diagnostic);
}

fn is_implicitly_typed(lambda: &Node, text: &str) -> bool {
    let Some(parameters) = lambda.child_by_field_name("parameters") else { return false };
    match parameters.kind() {
        "formal_parameters" => named_children(&parameters)
            .iter()
            .any(|p| p.child_by_field_name("type").is_some_and(|t| source(&t, text) == "var")),
        _ => true,
    }
}

/// An expression that may also be a statement, and so makes an expression lambda void-compatible.
fn is_statement_expression(expression: &Node) -> bool {
    matches!(
        expression.kind(),
        "method_invocation" | "object_creation_expression" | "assignment_expression" | "update_expression"
    )
}
