//! Rules of group I: imports, beyond the unused and unsorted ones the formatter fixes.

mod fully_qualified_type;
mod illegal_import;
mod star_import;
mod static_import;

use tree_sitter::Node;

use super::syntax::source;
use super::{Check, ConfiguredCheck, Rule};

pub(super) const CHECKS: &[(Rule, &[&str], Check)] = &[
    (Rule::StarImport, &["import_declaration"], star_import::check),
    (Rule::StaticImport, &["import_declaration"], static_import::check),
    (Rule::FullyQualifiedType, &["program"], fully_qualified_type::check),
];

/// The checks of rules with parameters.
pub(super) const CONFIGURED_CHECKS: &[(Rule, &[&str], ConfiguredCheck)] =
    &[(Rule::IllegalImport, &["import_declaration"], illegal_import::check)];

/// The imported name as Checkstyle spells it: without whitespace, ending in `.*` for an on-demand import.
fn imported_name(import: &Node, text: &str) -> String {
    let mut name = String::new();
    for child in import.children(&mut import.walk()) {
        match child.kind() {
            "identifier" | "scoped_identifier" => name.extend(source(&child, text).split_whitespace()),
            "asterisk" => name.push_str(".*"),
            _ => {}
        }
    }
    name
}

fn is_static(import: &Node) -> bool {
    import.children(&mut import.walk()).any(|c| c.kind() == "static")
}

fn is_on_demand(import: &Node) -> bool {
    import.children(&mut import.walk()).any(|c| c.kind() == "asterisk")
}

/// The offset of the last `.` of the imported name, where Checkstyle reports violations about the name.
fn last_dot(import: &Node) -> usize {
    let dot = if is_on_demand(import) {
        last_dot_child(import)
    } else {
        import.children(&mut import.walk()).find(|c| c.kind() == "scoped_identifier").and_then(|n| last_dot_child(&n))
    };
    dot.map_or(import.start_byte(), |d| d.start_byte())
}

fn last_dot_child<'t>(node: &Node<'t>) -> Option<Node<'t>> {
    node.children(&mut node.walk()).filter(|c| c.kind() == ".").last()
}
