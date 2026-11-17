//! S004: an `if` whose only statement is another `if`, neither with an `else` (PMD CollapsibleIfStatements):
//! `if (a) { if (b) s }` can be `if (a && b) s`. Reported on the inner `if`, like PMD.
//!
//! The fix joins the conditions, parenthesizing those whose operators bind less tightly than `&&` (`||`, `?:`,
//! assignments), and keeps the inner statement. It is left out if a comment outside the inner statement would be lost.

use tree_sitter::Node;

use super::comment_in;
use crate::lint::syntax::{is_comment, named_children, operator, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(outer: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if outer.child_by_field_name("alternative").is_some() {
        return;
    }
    let Some(consequence) = outer.child_by_field_name("consequence") else { return };
    let inner = match consequence.kind() {
        "if_statement" => consequence,
        "block" => match named_children(&consequence).into_iter().filter(|c| !is_comment(c)).collect::<Vec<_>>()[..] {
            [single] if single.kind() == "if_statement" => single,
            _ => return,
        },
        _ => return,
    };
    if inner.child_by_field_name("alternative").is_some() {
        return;
    }
    let diagnostic =
        Diagnostic::new(Rule::CollapsibleIf, inner.start_byte(), "This if statement could be combined with its parent");
    let fix = (|| {
        let statement = inner.child_by_field_name("consequence")?;
        let outer_condition = condition(outer, text)?;
        let inner_condition = condition(&inner, text)?;
        if comment_in(outer, outer.start_byte()..statement.start_byte())
            || comment_in(outer, statement.end_byte()..outer.end_byte())
        {
            return None;
        }
        let replacement = format!("if ({outer_condition} && {inner_condition}) {}", source(&statement, text));
        Some(Edit::new(outer.byte_range(), replacement))
    })();
    diagnostics.push(match fix {
        Some(fix) => diagnostic.with_fix(vec![fix]),
        None => diagnostic,
    });
}

/// The condition of an `if` statement as an operand of `&&`.
fn condition(statement: &Node, text: &str) -> Option<String> {
    let parenthesized = statement.child_by_field_name("condition")?;
    let expression = parenthesized.named_child(0)?;
    let code = source(&expression, text);
    let binds_less = match expression.kind() {
        "binary_expression" => operator(&expression, text) == "||",
        "ternary_expression" | "assignment_expression" | "lambda_expression" | "switch_expression" => true,
        _ => false,
    };
    Some(if binds_less { format!("({code})") } else { code.to_string() })
}
