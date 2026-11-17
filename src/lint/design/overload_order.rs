//! A008: overloaded methods or constructors that are not declared next to each other (Checkstyle
//! OverloadMethodsDeclarationOrder and ConstructorsDeclarationGrouping).
//!
//! As in Checkstyle, a method is reported if the previous method with its name is not directly before it, and
//! constructors (of classes, enums and records) after the first member that follows the first constructor. The
//! methods of annotation types and of enum constant bodies are not checked.

use std::collections::HashMap;

use tree_sitter::Node;

use super::{body, members_and_semicolons};
use crate::lint::syntax::source;
use crate::lint::{Diagnostic, Rule};

pub fn check(declaration: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(body) = body(declaration) else { return };
    let members = members_and_semicolons(&body);
    check_methods(&members, text, diagnostics);
    if matches!(declaration.kind(), "class_declaration" | "enum_declaration" | "record_declaration") {
        check_constructors(&members, diagnostics);
    }
}

fn check_methods(members: &[Node], text: &str, diagnostics: &mut Vec<Diagnostic>) {
    // The index in `members` and the line of the last method with each name.
    let mut last: HashMap<&str, (usize, usize)> = HashMap::new();
    for (i, member) in members.iter().enumerate() {
        if member.kind() != "method_declaration" {
            continue;
        }
        let name = member.child_by_field_name("name").map_or("", |n| source(&n, text));
        let line = member.start_position().row + 1;
        if let Some((previous, previous_line)) = last.get(name)
            && *previous + 1 != i
        {
            let message = format!(
                "All overloaded methods should be placed next to each other. Previous overloaded method located at \
                 line {previous_line}"
            );
            diagnostics.push(Diagnostic::new(Rule::OverloadOrder, member.start_byte(), message));
        }
        last.insert(name, (i, line));
    }
}

fn check_constructors(members: &[Node], diagnostics: &mut Vec<Diagnostic>) {
    let is_constructor = |m: &Node| matches!(m.kind(), "constructor_declaration" | "compact_constructor_declaration");
    let Some(first) = members.iter().position(is_constructor) else { return };
    let group_end = members[first..].iter().position(|m| !is_constructor(m)).map_or(members.len(), |i| first + i);
    let last_grouped = members[group_end - 1].start_position().row + 1;
    for constructor in members[group_end..].iter().filter(|m| is_constructor(m)) {
        let message = format!(
            "Constructors should be grouped together. The last grouped constructor is declared at line {last_grouped}"
        );
        diagnostics.push(Diagnostic::new(Rule::OverloadOrder, constructor.start_byte(), message));
    }
}
