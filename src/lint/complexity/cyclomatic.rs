//! C001: a method with too many decision points (Checkstyle CyclomaticComplexity, PMD CyclomaticComplexity).
//!
//! As in Checkstyle (with `switchBlockAsSingleDecisionPoint=false`), the complexity of a method, constructor or
//! initializer is 1 plus the number of `if`, `while`, `do`, `for`, `case` (one per `case` keyword), `catch`, `?:`,
//! `&&`, `||` and `when`. Code in lambdas counts for the enclosing method, code in methods of local and anonymous
//! classes only for those.

use tree_sitter::Node;

use super::{Event, Visitor, is_method_like, kinds};
use crate::lint::syntax::operator;
use crate::lint::{Diagnostic, Rule};

pub fn visitor<'t, 'a>(text: &'a str, max: u64, diagnostics: &'a mut Vec<Diagnostic>) -> Visitor<'t, 'a> {
    // The complexity of each enclosing method; the first entry collects the code outside methods.
    let mut stack: Vec<u64> = vec![1];
    let kinds = kinds![
        "method_declaration",
        "constructor_declaration",
        "compact_constructor_declaration",
        "static_initializer",
        "block",
        "if_statement",
        "while_statement",
        "do_statement",
        "for_statement",
        "enhanced_for_statement",
        "catch_clause",
        "ternary_expression",
        "guard",
        "binary_expression",
        "case",
    ];
    let visit = move |event: Event, node: &Node<'t>, ancestors: &[Node<'t>]| {
        if is_method_like(node, ancestors.last()) {
            match event {
                Event::Enter => stack.push(1),
                Event::Leave => {
                    let complexity = stack.pop().unwrap_or(1);
                    if complexity > max {
                        let message = format!("Cyclomatic Complexity is {complexity} (max allowed is {max}).");
                        diagnostics.push(Diagnostic::new(Rule::CyclomaticComplexity, node.start_byte(), message));
                    }
                }
            }
        } else if event == Event::Enter
            && is_decision_point(node, text)
            && let Some(top) = stack.last_mut()
        {
            *top += 1;
        }
    };
    (kinds, Box::new(visit))
}

fn is_decision_point(node: &Node, text: &str) -> bool {
    match node.kind() {
        "if_statement"
        | "while_statement"
        | "do_statement"
        | "for_statement"
        | "enhanced_for_statement"
        | "catch_clause"
        | "ternary_expression"
        | "guard" => true,
        "binary_expression" => matches!(operator(node, text), "&&" | "||"),
        "case" => !node.is_named(),
        _ => false,
    }
}
