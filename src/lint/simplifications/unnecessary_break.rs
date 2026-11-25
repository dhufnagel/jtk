//! S056: `break;` as the last statement of a rule's block in a switch statement (Error Prone
//! UnnecessaryBreakInSwitch, IntelliJ UnnecessaryBreak): `case 1 -> { f(); break; }` cannot fall through anyway.
//!
//! The fix removes the `break`, and writes the rule as `case 1 -> f();` if one expression statement is left. Both
//! change nothing. A `break` in an `if` at the end of the block, and the last `break` of an old-style switch, which
//! many keep on purpose, are not reported.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{CaseKind, K, Tree};

use super::line_removal;

pub fn check(statement: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if !matches!(statement.kind, K::Break(None)) {
        return;
    }
    let (Some(block), Some(switch)) = (context.parent(), context.grandparent()) else { return };
    let K::Block { stmts, .. } = &block.kind else { return };
    let K::Switch { cases, .. } = &switch.kind else { return };
    let is_rule_body =
        cases.iter().any(|c| c.kind == CaseKind::Rule && c.body.as_deref().is_some_and(|b| std::ptr::eq(b, block)));
    if !is_rule_body || !stmts.last().is_some_and(|s| std::ptr::eq(s, statement)) {
        return;
    }
    let diagnostic = Diagnostic::new(Rule::UnnecessaryBreak, statement.start, "Unnecessary break statement");
    let tokens = context.tokens();
    let fix = match stmts.as_slice() {
        [only @ Tree { kind: K::ExprStmt(_), .. }, _] if !tokens.has_comment(block.start..block.end) => {
            Edit::new(block.start..block.end, context.source(only))
        }
        _ if !tokens.has_comment(statement.start..statement.end) => {
            line_removal(statement.start..statement.end, context.text)
        }
        _ => {
            diagnostics.push(diagnostic);
            return;
        }
    };
    diagnostics.push(diagnostic.with_fix(vec![fix]));
}
