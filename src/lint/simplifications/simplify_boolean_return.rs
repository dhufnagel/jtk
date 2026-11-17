//! S002: `if (c) return true; else return false;` (Checkstyle SimplifyBooleanReturn, PMD SimplifyBooleanReturns):
//! can be `return c;`. Fixed when the two literals differ.

use tree_sitter::Node;

use crate::lint::syntax::{is_boolean_literal, is_comment, named_children, negation, source, strip_parentheses};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(statement: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (Some(condition), Some(consequence), Some(alternative)) = (
        statement.child_by_field_name("condition"),
        statement.child_by_field_name("consequence"),
        statement.child_by_field_name("alternative"),
    ) else {
        return;
    };
    let (Some((keyword, when_true)), Some((_, when_false))) =
        (returned_literal(&consequence, text), returned_literal(&alternative, text))
    else {
        return;
    };
    let diagnostic =
        Diagnostic::new(Rule::SimplifyBooleanReturn, statement.start_byte(), "Conditional return of boolean literals");
    let simplified = match (when_true, when_false) {
        ("true", "false") => source(&strip_parentheses(&condition), text).to_string(),
        ("false", "true") => negation(&condition, text),
        _ => {
            diagnostics.push(diagnostic);
            return;
        }
    };
    let fix = Edit::new(statement.byte_range(), format!("{keyword} {simplified};"));
    diagnostics.push(diagnostic.with_fix(vec![fix]));
}

/// `return true;` or `{ yield false; }`: the keyword and the literal.
fn returned_literal<'a>(statement: &Node, text: &'a str) -> Option<(&'a str, &'a str)> {
    let statement = match statement.kind() {
        "block" => {
            match named_children(statement).into_iter().filter(|c| !is_comment(c)).collect::<Vec<_>>().as_slice() {
                [single] => *single,
                _ => return None,
            }
        }
        _ => *statement,
    };
    if !matches!(statement.kind(), "return_statement" | "yield_statement") {
        return None;
    }
    let value = strip_parentheses(&statement.named_child(0)?);
    let keyword = if statement.kind() == "return_statement" { "return" } else { "yield" };
    is_boolean_literal(&value).then(|| (keyword, source(&value, text)))
}
