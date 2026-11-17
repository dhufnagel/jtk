//! S018: blocks nested directly in other blocks (Checkstyle AvoidNestedBlocks with the default
//! `allowInSwitchCase=false`, so also `case 1: { ... }`), which usually only limit the scope of variables.

use tree_sitter::Node;

use crate::lint::syntax::named_children;
use crate::lint::{Diagnostic, Rule};

pub fn check(container: &Node, _text: &str, diagnostics: &mut Vec<Diagnostic>) {
    for block in named_children(container).iter().filter(|c| c.kind() == "block") {
        diagnostics.push(Diagnostic::new(Rule::NestedBlock, block.start_byte(), "Avoid nested blocks"));
    }
}
