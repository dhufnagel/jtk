//! S001: control statement body without braces (Checkstyle NeedBraces, PMD ControlStatementBraces, Eclipse "Use
//! blocks"). `else if` is allowed.

use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(statement: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (keyword, bodies): (&str, [Option<&Tree>; 2]) = match &statement.kind {
        K::If { then, else_, .. } => ("if", [Some(then), else_.as_deref()]),
        K::While { body, .. } => ("while", [Some(body), None]),
        K::DoWhile { body, .. } => ("do", [Some(body), None]),
        K::For { body, .. } | K::ForEach { body, .. } => ("for", [Some(body), None]),
        _ => return,
    };
    for (index, body) in bodies.into_iter().enumerate() {
        let Some(body) = body else { continue };
        let is_else = index == 1;
        if matches!(body.kind, K::Block { .. }) || is_else && matches!(body.kind, K::If { .. }) {
            continue;
        }
        let (name, position) = match is_else {
            true => ("else", context.tokens().previous(body.start).map_or(statement.start, |t| t.start)),
            false => (keyword, statement.start),
        };
        let message = format!("'{name}' body without braces");
        let braced = Edit::new(body.start..body.end, format!("{{ {} }}", context.source(body)));
        diagnostics.push(Diagnostic::new(Rule::MissingBraces, position, message).with_fix(vec![braced]));
    }
}
