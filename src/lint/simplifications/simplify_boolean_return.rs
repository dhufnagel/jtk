//! S002: `if (c) return true; else return false;` (Checkstyle SimplifyBooleanReturn, PMD SimplifyBooleanReturns):
//! can be `return c;`. Fixed when the two literals differ.

use crate::ast::{K, Tree};
use crate::lint::tree::{Context, is_boolean_literal, negation, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::If { cond, then, else_: Some(alternative) } = &tree.kind else { return };
    let (Some((keyword, when_true)), Some((_, when_false))) =
        (returned_literal(then, context), returned_literal(alternative, context))
    else {
        return;
    };
    let diagnostic = Diagnostic::new(Rule::SimplifyBooleanReturn, tree.start, "Conditional return of boolean literals");
    let simplified = match (when_true, when_false) {
        ("true", "false") => context.source(strip_parens(cond)).to_string(),
        ("false", "true") => negation(cond, context.text),
        _ => {
            diagnostics.push(diagnostic);
            return;
        }
    };
    let fix = Edit::new(tree.start..tree.end, format!("{keyword} {simplified};"));
    diagnostics.push(diagnostic.with_fix(vec![fix]));
}

/// `return true;` or `{ yield false; }`: the keyword and the literal.
fn returned_literal<'t>(statement: &'t Tree, context: &Context<'t>) -> Option<(&'static str, &'t str)> {
    let statement = match &statement.kind {
        K::Block { stmts, .. } => match stmts.iter().filter(|s| !matches!(s.kind, K::Empty)).collect::<Vec<_>>()[..] {
            [single] => single,
            _ => return None,
        },
        _ => statement,
    };
    let (keyword, value) = match &statement.kind {
        K::Return(Some(value)) => ("return", value),
        K::Yield(value) => ("yield", value),
        _ => return None,
    };
    // tree-sitter takes a comment before the value for the value.
    if context.tokens().has_comment(statement.start..value.start) {
        return None;
    }
    let value = strip_parens(value);
    is_boolean_literal(value).then(|| (keyword, context.source(value)))
}
