//! S007: semicolons that do nothing (Checkstyle EmptyStatement and UnnecessarySemicolon*, PMD UnnecessarySemicolon):
//! after a type or member declaration, after the last enum constant, after the last try resource, and empty
//! statements. An empty loop or `if` body is replaced by `{}`.

use tree_sitter::Node;

use crate::lint::syntax::named_children;
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(node: &Node, _text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let semicolons: Vec<Node> = node.children(&mut node.walk()).filter(|c| c.kind() == ";").collect();
    match node.kind() {
        "program" => report_all(&semicolons, "Unnecessary semicolon after type declaration", diagnostics),
        "class_body" | "interface_body" | "annotation_type_body" => {
            report_all(&semicolons, "Unnecessary semicolon after member declaration", diagnostics)
        }
        "enum_body_declarations" => {
            // The first semicolon ends the constants, and is needed if declarations follow.
            let Some((first, rest)) = semicolons.split_first() else { return };
            if named_children(node).is_empty() {
                report_all(&[*first], "Unnecessary semicolon after the last enum constant", diagnostics);
            }
            report_all(rest, "Unnecessary semicolon after member declaration", diagnostics);
        }
        "resource_specification" => {
            let last_child = node.child(node.child_count().saturating_sub(2));
            let trailing = semicolons.iter().filter(|s| Some(**s) == last_child).copied().collect::<Vec<_>>();
            report_all(&trailing, "Unnecessary semicolon after the last resource", diagnostics);
        }
        "block" | "constructor_body" | "switch_block_statement_group" => {
            report_all(&semicolons, "Empty statement", diagnostics)
        }
        "if_statement" => report_empty_bodies(&semicolons, diagnostics),
        "while_statement" | "for_statement" | "enhanced_for_statement" | "labeled_statement" => {
            let last = node.child(node.child_count().saturating_sub(1));
            report_empty_bodies(&semicolons.into_iter().filter(|s| Some(*s) == last).collect::<Vec<_>>(), diagnostics);
        }
        "do_statement" => {
            let after_do = node.child(1);
            report_empty_bodies(
                &semicolons.into_iter().filter(|s| Some(*s) == after_do).collect::<Vec<_>>(),
                diagnostics,
            );
        }
        _ => {}
    }
}

fn report_all(semicolons: &[Node], message: &str, diagnostics: &mut Vec<Diagnostic>) {
    for semicolon in semicolons {
        let fix = Edit::new(semicolon.byte_range(), "");
        diagnostics
            .push(Diagnostic::new(Rule::UnnecessarySemicolon, semicolon.start_byte(), message).with_fix(vec![fix]));
    }
}

/// A `;` that is the body of a statement: replaced by `{}`, as removing it would change the statement.
fn report_empty_bodies(semicolons: &[Node], diagnostics: &mut Vec<Diagnostic>) {
    for semicolon in semicolons {
        let fix = Edit::new(semicolon.byte_range(), "{}");
        let diagnostic = Diagnostic::new(Rule::UnnecessarySemicolon, semicolon.start_byte(), "Empty statement");
        diagnostics.push(diagnostic.with_fix(vec![fix]));
    }
}
