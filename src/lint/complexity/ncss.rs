//! C004: a method, class, record or file with too many non-commenting source statements (Checkstyle JavaNCSS, PMD
//! NcssCount).
//!
//! As in Checkstyle, counts package and (non-static) import declarations, class, interface and record declarations, methods,
//! constructors and initializers, field declarations and local variable declarations (one per statement),
//! expression statements (not the expression body of a switch rule), explicit constructor calls, `if`, `else`,
//! loops, `switch`, `case` and `default` labels (also the `default` of interface methods and annotation elements),
//! `break`, `continue`, `return`, `throw`, `synchronized`, `catch`, `finally` and labeled statements. Each count goes
//! to the enclosing method, class (not interface or enum), record and the file.

use tree_sitter::Node;

use super::{Event, Visitor, file_position, is_method_like, kinds};
use crate::lint::options::JavaNcss;
use crate::lint::{Diagnostic, Rule};

pub fn visitor<'t, 'a>(_text: &'a str, limits: JavaNcss, diagnostics: &'a mut Vec<Diagnostic>) -> Visitor<'t, 'a> {
    // The counts of the enclosing methods, classes and records; the first is the file's.
    let mut counters: Vec<u32> = vec![0];
    let kinds = kinds![
        "method_declaration",
        "constructor_declaration",
        "compact_constructor_declaration",
        "static_initializer",
        "block",
        "class_declaration",
        "record_declaration",
        "interface_declaration",
        "package_declaration",
        "import_declaration",
        "explicit_constructor_invocation",
        "if_statement",
        "while_statement",
        "do_statement",
        "for_statement",
        "enhanced_for_statement",
        "switch_expression",
        "break_statement",
        "continue_statement",
        "return_statement",
        "throw_statement",
        "synchronized_statement",
        "catch_clause",
        "finally_clause",
        "labeled_statement",
        "local_variable_declaration",
        "field_declaration",
        "constant_declaration",
        "expression_statement",
        "else",
        "case",
        "default",
        "program",
    ];
    let visit = move |event: Event, node: &Node<'t>, ancestors: &[Node<'t>]| {
        let parent = ancestors.last();
        if node.kind() == "program" {
            let count = counters[0];
            if event == Event::Leave && count > limits.file_maximum {
                let message = format!("NCSS for this file is {count} (max allowed is {}).", limits.file_maximum);
                diagnostics.push(Diagnostic::new(Rule::JavaNcss, file_position(node), message));
            }
            return;
        }
        let is_counter =
            is_method_like(node, parent) || matches!(node.kind(), "class_declaration" | "record_declaration");
        match event {
            Event::Enter => {
                if is_counter {
                    counters.push(0);
                }
                if is_counted(node, parent) {
                    for count in &mut counters {
                        *count += 1;
                    }
                }
            }
            Event::Leave if is_counter && counters.len() > 1 => {
                let Some(count) = counters.pop() else { return };
                let (what, max) = match node.kind() {
                    "class_declaration" => ("class", limits.class_maximum),
                    "record_declaration" => ("record", limits.record_maximum),
                    _ => ("method", limits.method_maximum),
                };
                if count > max {
                    let message = format!("NCSS for this {what} is {count} (max allowed is {max}).");
                    diagnostics.push(Diagnostic::new(Rule::JavaNcss, node.start_byte(), message));
                }
            }
            Event::Leave => {}
        }
    };
    (kinds, Box::new(visit))
}

fn is_counted(node: &Node, parent: Option<&Node>) -> bool {
    let parent_kind = parent.map_or("", |p| p.kind());
    if !node.is_named() {
        return match node.kind() {
            "else" => parent_kind == "if_statement",
            "case" => parent_kind == "switch_label",
            "default" => true,
            _ => false,
        };
    }
    match node.kind() {
        "import_declaration" => !node.children(&mut node.walk()).any(|c| c.kind() == "static"),
        "package_declaration"
        | "class_declaration"
        | "interface_declaration"
        | "record_declaration"
        | "explicit_constructor_invocation"
        | "if_statement"
        | "while_statement"
        | "do_statement"
        | "for_statement"
        | "enhanced_for_statement"
        | "switch_expression"
        | "break_statement"
        | "continue_statement"
        | "return_statement"
        | "throw_statement"
        | "synchronized_statement"
        | "catch_clause"
        | "finally_clause"
        | "labeled_statement" => true,
        "method_declaration" | "constructor_declaration" | "compact_constructor_declaration" | "static_initializer" => {
            true
        }
        "block" => is_method_like(node, parent),
        "local_variable_declaration" => {
            matches!(parent_kind, "block" | "constructor_body" | "switch_block_statement_group")
        }
        "field_declaration" | "constant_declaration" => true,
        "expression_statement" => parent_kind != "switch_rule",
        _ => false,
    }
}
