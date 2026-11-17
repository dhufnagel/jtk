//! B005: a `switch` statement without `default` (Checkstyle MissingSwitchDefault, PMD SwitchStmtsShouldHaveDefault
//! and NonExhaustiveSwitch): a value no `case` handles is silently ignored.
//!
//! As in Checkstyle, switch expressions are not checked (the compiler checks that they are exhaustive), nor are
//! switch statements with an unguarded pattern or a `case null`, which must be exhaustive too.

use tree_sitter::Node;

use crate::lint::syntax::named_children;
use crate::lint::{Diagnostic, Rule};

/// Checks the switch statements among the statement's children: Checkstyle takes a `switch` elsewhere for a switch
/// expression.
pub fn check(parent: &Node, _text: &str, diagnostics: &mut Vec<Diagnostic>) {
    for switch in named_children(parent).into_iter().filter(|c| c.kind() == "switch_expression") {
        let labels = labels(&switch);
        let is_exempt = labels.iter().any(|label| {
            let children = named_children(label);
            let is_default = label.children(&mut label.walk()).any(|c| c.kind() == "default");
            let has_pattern =
                children.iter().any(|c| c.kind() == "pattern") && !children.iter().any(|c| c.kind() == "guard");
            let has_null = children.iter().any(|c| c.kind() == "null_literal");
            is_default || has_pattern || has_null
        });
        if !is_exempt {
            let message = "switch without \"default\" clause";
            diagnostics.push(Diagnostic::new(Rule::MissingSwitchDefault, switch.start_byte(), message));
        }
    }
}

fn labels<'t>(switch: &Node<'t>) -> Vec<Node<'t>> {
    let Some(body) = switch.child_by_field_name("body") else { return Vec::new() };
    named_children(&body).iter().flat_map(named_children).filter(|c| c.kind() == "switch_label").collect()
}
