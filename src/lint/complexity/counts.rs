//! Counts of declared things, as in Checkstyle:
//!
//! - C010: parameters of a method or constructor (Checkstyle ParameterNumber, also for overriding methods),
//! - C011: components of a record (Checkstyle RecordComponentNumber),
//! - C013: top-level types of a file (Checkstyle OuterTypeNumber), reported at the first declaration,
//! - C021: exceptions in a `throws` clause (Checkstyle ThrowsCount), except of private methods and methods annotated
//!   with `@Override`.

use super::{file_position, token_before};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Modifier, Tree};

pub fn check_parameters(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    let max = context.options.parameter_number.max as usize;
    let count = method.params.len() + usize::from(method.receiver.is_some());
    if count > max {
        let message = format!("More than {max} parameters (found {count}).");
        diagnostics.push(Diagnostic::new(Rule::ParameterNumber, method.name_pos, message));
    }
}

pub fn check_record_components(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    let max = context.options.record_component_number.max as usize;
    let count = class.record_components.len();
    if class.kind == ClassKind::Record && count > max {
        let message = format!("Number of record components is {count} (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::RecordComponentNumber, tree.start, message));
    }
}

pub fn check_outer_types(unit: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::CompilationUnit { types, .. } = &unit.kind else { return };
    let max = context.options.outer_type_number.max as usize;
    // Not the methods and fields of a compact source file.
    let count = types.iter().filter(|t| matches!(t.kind, K::Class(_))).count();
    if count > max {
        let message = format!("Outer types defined is {count} (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::OuterTypeNumber, file_position(context), message));
    }
}

pub fn check_throws(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    let max = context.options.throws_count.max as usize;
    let count = method.throws.len();
    if count <= max || method.mods.has(Modifier::Private) || method.mods.annotations.iter().any(is_override) {
        return;
    }
    let throws = token_before(context, method.throws[0].start, method.throws[0].start);
    let message = format!("Throws count is {count} (max allowed is {max}).");
    diagnostics.push(Diagnostic::new(Rule::ThrowsCount, throws, message));
}

/// Whether the annotation is `@Override`, which Checkstyle recognizes by the last name of a qualified annotation with
/// more than two names (`@java.lang.Override`) and by the first name of one with two names.
fn is_override(annotation: &Tree) -> bool {
    let K::Annotation { ty, .. } = &annotation.kind else { return false };
    let mut names = Vec::new();
    let mut name = &**ty;
    while let K::MemberSelect { expr, name: last } = &name.kind {
        names.push(last.as_str());
        name = expr;
    }
    if let K::Ident(first) = &name.kind {
        names.push(first);
    }
    // The names are in reverse order.
    let checked = match names.as_slice() {
        [_, first] => first,
        [last, ..] => last,
        [] => return false,
    };
    *checked == "Override"
}
