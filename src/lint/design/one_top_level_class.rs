//! A006: more than one top-level type in a file (Checkstyle OneTopLevelClass).
//!
//! As in Checkstyle, the public type (or else the first one) may stay; every other top-level type is reported.

use tree_sitter::Node;

use super::{has_keyword, is_type_declaration};
use crate::lint::syntax::{named_children, source};
use crate::lint::{Diagnostic, Rule};

pub fn check(program: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let types: Vec<Node> = named_children(program).into_iter().filter(is_type_declaration).collect();
    let Some(first) = types.first() else { return };
    let is_public = |t: &Node| has_keyword(t, text, "public");
    let mut report = |t: &Node| {
        let name = t.child_by_field_name("name").map_or("", |n| source(&n, text));
        let message = format!("Top-level class {name} has to reside in its own source file");
        diagnostics.push(Diagnostic::new(Rule::OneTopLevelClass, t.start_byte(), message));
    };
    if !is_public(first) && types.iter().any(is_public) {
        report(first);
    }
    for t in types.iter().skip(1).filter(|t| !is_public(t)) {
        report(t);
    }
}
