//! M014 and M016: the `permits` clause of sealed types, Java 17.
//!
//! - M014: a `sealed` class or interface without `permits` (Checkstyle SealedShouldHavePermitsList). No fix.
//! - M016: a `permits` clause that only names types declared in the same file (Checkstyle UnnecessaryPermitsClause):
//!   the compiler infers it. As in Checkstyle, types are compared by simple name with all classes, interfaces, enums
//!   and records of the file. The fix removes the clause.

use tree_sitter::Node;

use crate::lint::syntax::{for_each_descendant, has_modifier, named_children, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn missing_permits(declaration: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if has_modifier(declaration, text, "sealed") && declaration.child_by_field_name("permits").is_none() {
        let message = "Sealed classes or interfaces should explicitly declare permitted subclasses";
        diagnostics.push(Diagnostic::new(Rule::SealedPermits, declaration.start_byte(), message));
    }
}

pub fn unnecessary_permits(program: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    // Most files have no `permits` clause; finding that in the text is much cheaper than walking the tree.
    if !text.contains("permits") {
        return;
    }
    let mut names = Vec::new();
    let mut clauses = Vec::new();
    for_each_descendant(program, &[], |node| match node.kind() {
        "class_declaration" | "interface_declaration" | "enum_declaration" | "record_declaration" => {
            names.extend(node.child_by_field_name("name").map(|n| source(&n, text)));
        }
        "permits" => clauses.push(*node),
        _ => {}
    });
    for clause in clauses {
        let mut permitted = Vec::new();
        for_each_descendant(&clause, &["scoped_type_identifier"], |node| match node.kind() {
            "type_identifier" => permitted.push(source(node, text)),
            "scoped_type_identifier" => {
                permitted.extend(named_children(node).last().map(|n| source(n, text)));
            }
            _ => {}
        });
        if permitted.iter().all(|p| names.contains(p)) {
            // Only the blanks on the same line: a line comment may end the line before.
            let before = text[..clause.start_byte()].trim_end_matches([' ', '\t']).len();
            let fix = Edit::new(before..clause.end_byte(), "");
            let message = "Unnecessary 'permits' clause for classes in the same compilation unit";
            diagnostics
                .push(Diagnostic::new(Rule::UnnecessaryPermits, clause.start_byte(), message).with_fix(vec![fix]));
        }
    }
}
