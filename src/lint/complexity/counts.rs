//! Counts of declared things, as in Checkstyle:
//!
//! - C010: parameters of a method or constructor (Checkstyle ParameterNumber, also for overriding methods),
//! - C011: components of a record (Checkstyle RecordComponentNumber),
//! - C013: top-level types of a file (Checkstyle OuterTypeNumber), reported at the first declaration,
//! - C021: exceptions in a `throws` clause (Checkstyle ThrowsCount), except of private methods and methods annotated
//!   with `@Override`.

use tree_sitter::Node;

use super::{file_position, is_type_declaration};
use crate::lint::syntax::{has_modifier, is_comment, modifiers, source};
use crate::lint::{Diagnostic, Rule, RuleOptions};

pub fn check_parameters(method: &Node, _text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let max = options.parameter_number.max as usize;
    let count = method.child_by_field_name("parameters").map_or(0, |p| parameter_count(&p));
    if count > max {
        let name = method.child_by_field_name("name").map_or(method.start_byte(), |n| n.start_byte());
        let message = format!("More than {max} parameters (found {count}).");
        diagnostics.push(Diagnostic::new(Rule::ParameterNumber, name, message));
    }
}

pub fn check_record_components(record: &Node, _text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let max = options.record_component_number.max as usize;
    let count = record.child_by_field_name("parameters").map_or(0, |p| parameter_count(&p));
    if count > max {
        let message = format!("Number of record components is {count} (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::RecordComponentNumber, record.start_byte(), message));
    }
}

fn parameter_count(parameters: &Node) -> usize {
    let mut cursor = parameters.walk();
    parameters
        .named_children(&mut cursor)
        .filter(|p| matches!(p.kind(), "formal_parameter" | "spread_parameter" | "receiver_parameter"))
        .count()
}

pub fn check_outer_types(program: &Node, _text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let max = options.outer_type_number.max as usize;
    let count = program.named_children(&mut program.walk()).filter(is_type_declaration).count();
    if count > max {
        let message = format!("Outer types defined is {count} (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::OuterTypeNumber, file_position(program), message));
    }
}

pub fn check_throws(method: &Node, text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let max = options.throws_count.max as usize;
    let Some(throws) = method.named_children(&mut method.walk()).find(|c| c.kind() == "throws") else { return };
    if has_modifier(method, text, "private") || is_overriding(method, text) {
        return;
    }
    let count = throws.named_children(&mut throws.walk()).filter(|c| !is_comment(c) && !is_annotation(c)).count();
    if count > max {
        let message = format!("Throws count is {count} (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::ThrowsCount, throws.start_byte(), message));
    }
}

fn is_annotation(node: &Node) -> bool {
    matches!(node.kind(), "annotation" | "marker_annotation")
}

/// Whether the method is annotated with `@Override`, which Checkstyle recognizes by the last name of a qualified
/// annotation with more than two names (`@java.lang.Override`) and by the first name of one with two names.
fn is_overriding(method: &Node, text: &str) -> bool {
    let Some(modifiers) = modifiers(method) else { return false };
    modifiers.named_children(&mut modifiers.walk()).filter(is_annotation).any(|annotation| {
        let name = annotation.child_by_field_name("name").map_or("", |n| source(&n, text));
        let parts: Vec<&str> = name.split('.').map(str::trim).collect();
        let checked = if parts.len() == 2 { parts[0] } else { parts[parts.len() - 1] };
        checked == "Override"
    })
}
