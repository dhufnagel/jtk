//! B006: a `default` label before other `case`s of the switch (Checkstyle DefaultComesLast with its default
//! properties, PMD DefaultLabelNotLastInSwitch): readers expect the fallback at the end.
//!
//! As in Checkstyle, `default` may share its statements with `case` labels in any order if they are the last ones
//! (`default: case 1: ...`), and `case null, default` is not checked.

use tree_sitter::Node;

use crate::lint::syntax::{is_comment, named_children};
use crate::lint::{Diagnostic, Rule};

pub fn check(switch_block: &Node, _text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let groups: Vec<Node> = named_children(switch_block).into_iter().filter(|c| !is_comment(c)).collect();
    // Checkstyle's case groups: tree-sitter splits labels that share statements into groups without statements.
    let ends_case_group = |group: &Node| {
        group.kind() == "switch_rule"
            || named_children(group).iter().any(|c| c.kind() != "switch_label" && !is_comment(c))
    };
    for (index, group) in groups.iter().enumerate() {
        let is_default = |c: &Node| c.kind() == "switch_label" && c.child(0).is_some_and(|k| k.kind() == "default");
        let Some(label) = named_children(group).into_iter().find(is_default) else { continue };
        let group_end = groups[index..].iter().position(ends_case_group).map_or(groups.len() - 1, |p| index + p);
        if group_end + 1 < groups.len() {
            diagnostics.push(Diagnostic::new(
                Rule::DefaultComesLast,
                label.start_byte(),
                "Default should be last label in the switch",
            ));
        }
    }
}
