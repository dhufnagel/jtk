//! Nesting depths, as in Checkstyle across the whole file (a statement in a lambda or local class within another one
//! counts as nested):
//!
//! - C014: `if` statements (Checkstyle NestedIfDepth, PMD AvoidDeeplyNestedIfStmts); an `else if`, also written as an
//!   `else` block holding only the `if`, does not nest,
//! - C015: `for` loops (Checkstyle NestedForDepth),
//! - C016: `try` statements (Checkstyle NestedTryDepth).

use tree_sitter::Node;

use super::{Event, KindSet, Visitor, code_children, kinds};
use crate::lint::{Diagnostic, Rule};

pub fn visitor_if<'t, 'a>(_text: &'a str, max: usize, diagnostics: &'a mut Vec<Diagnostic>) -> Visitor<'t, 'a> {
    let report = Report { rule: Rule::NestedIfDepth, name: "if-else", max };
    depth_visitor(kinds!["if_statement"], report, is_else_if, diagnostics)
}

pub fn visitor_for<'t, 'a>(_text: &'a str, max: usize, diagnostics: &'a mut Vec<Diagnostic>) -> Visitor<'t, 'a> {
    let report = Report { rule: Rule::NestedForDepth, name: "for", max };
    depth_visitor(kinds!["for_statement", "enhanced_for_statement"], report, |_, _| false, diagnostics)
}

pub fn visitor_try<'t, 'a>(_text: &'a str, max: usize, diagnostics: &'a mut Vec<Diagnostic>) -> Visitor<'t, 'a> {
    let report = Report { rule: Rule::NestedTryDepth, name: "try", max };
    depth_visitor(kinds!["try_statement", "try_with_resources_statement"], report, |_, _| false, diagnostics)
}

struct Report {
    rule: Rule,
    /// The name of the statement in the message.
    name: &'static str,
    max: usize,
}

/// Reports the statements of the given kinds nested in more than `max` others, counting only statements that are
/// not `continues` of their parent (an `else if` continues the chain of its `if` rather than nesting in it).
fn depth_visitor<'t, 'a>(
    kinds: &'static KindSet,
    report: Report,
    continues: fn(&Node, &[Node]) -> bool,
    diagnostics: &'a mut Vec<Diagnostic>,
) -> Visitor<'t, 'a> {
    let mut depth = 0;
    let visit = move |event: Event, node: &Node<'t>, ancestors: &[Node<'t>]| {
        if continues(node, ancestors) {
            return;
        }
        match event {
            Event::Enter => {
                if depth > report.max {
                    let message = format!("Nested {} depth is {depth} (max allowed is {}).", report.name, report.max);
                    diagnostics.push(Diagnostic::new(report.rule, node.start_byte(), message));
                }
                depth += 1;
            }
            Event::Leave => depth -= 1,
        }
    };
    (kinds, Box::new(visit))
}

/// Whether the `if` is the alternative of another one, directly or as the only statement of an `else` block.
fn is_else_if(node: &Node, ancestors: &[Node]) -> bool {
    let alternative_of_if = |alternative: &Node, ancestors: &[Node]| {
        ancestors
            .last()
            .is_some_and(|p| p.kind() == "if_statement" && p.child_by_field_name("alternative") == Some(*alternative))
    };
    match ancestors.split_last() {
        Some((parent, rest)) if parent.kind() == "block" => {
            let children = code_children(parent);
            children.len() == 3 && children[1] == *node && alternative_of_if(parent, rest)
        }
        _ => alternative_of_if(node, ancestors),
    }
}
