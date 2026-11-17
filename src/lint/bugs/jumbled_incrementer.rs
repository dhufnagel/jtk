//! B015: a nested `for` loop that increments the outer loop's variable (PMD JumbledIncrementer).

use tree_sitter::Node;

use super::modified_control_variable::{loop_variables, updated_variables};
use crate::lint::syntax::NESTED_CODE;
use crate::lint::{Diagnostic, Rule};

pub fn check(statement: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let own = loop_variables(statement, text);
    let updated: Vec<&str> = updated_variables(statement, text).into_iter().filter(|v| !own.contains(v)).collect();
    let outer_loops = std::iter::successors(statement.parent(), |n| n.parent())
        .take_while(|n| !NESTED_CODE.contains(&n.kind()))
        .filter(|n| n.kind() == "for_statement");
    for outer in outer_loops {
        if let Some(variable) = updated.iter().find(|v| loop_variables(&outer, text).contains(v)) {
            let message = format!("Inner loop updates the outer loop's variable '{variable}'");
            diagnostics.push(Diagnostic::new(Rule::JumbledIncrementer, statement.start_byte(), message));
            return;
        }
    }
}
