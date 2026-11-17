//! B032: a `switch` over a reference type without `case null` (Checkstyle MissingNullCaseInSwitch): it throws a
//! `NullPointerException` for `null`.
//!
//! As in Checkstyle, a switch counts as over a reference type if a `case` has a pattern or a string literal (not a
//! text block), and only the first constant of a `case` is checked for `null` (`case null, default` but not
//! `case "a", null`).

use tree_sitter::Node;

use crate::lint::syntax::{for_each_descendant, is_comment, source};
use crate::lint::{Diagnostic, Rule};

pub fn check(switch: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(body) = switch.child_by_field_name("body") else { return };
    let labels: Vec<Node> = body
        .named_children(&mut body.walk())
        .filter(|c| matches!(c.kind(), "switch_block_statement_group" | "switch_rule"))
        .flat_map(|c| c.named_children(&mut c.walk()).filter(|l| l.kind() == "switch_label").collect::<Vec<_>>())
        .filter(|l| l.child(0).is_some_and(|keyword| keyword.kind() == "case"))
        .collect();
    let has_null = labels
        .iter()
        .any(|l| l.named_children(&mut l.walk()).find(|c| !is_comment(c)).is_some_and(|c| c.kind() == "null_literal"));
    if !has_null && labels.iter().any(|l| has_pattern(l) || has_string(l, text)) {
        diagnostics.push(Diagnostic::new(
            Rule::MissingNullCase,
            switch.start_byte(),
            "Switch using reference types should have a null case",
        ));
    }
}

fn has_pattern(label: &Node) -> bool {
    label.named_children(&mut label.walk()).any(|c| matches!(c.kind(), "pattern" | "type_pattern" | "record_pattern"))
}

fn has_string(label: &Node, text: &str) -> bool {
    let mut found = false;
    for_each_descendant(label, &[], |n| {
        found |= n.kind() == "string_literal" && !source(n, text).starts_with("\"\"\"");
    });
    found
}
