//! B003: empty `catch` block (Checkstyle EmptyCatchBlock, PMD EmptyCatchBlock, Error Prone EmptyCatch).
//!
//! A block with only `;` counts as empty. A comment in the block, or naming the exception like PMD's defaults
//! (`ignored`, `expected`) or `_`, documents that the exception is swallowed on purpose.

use tree_sitter::Node;

use crate::lint::{Diagnostic, Rule, source};

const INTENTIONAL_NAMES: &[&str] = &["ignored", "expected", "_"];

pub fn check(catch: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(body) = catch.child_by_field_name("body") else { return };
    if body.named_child_count() > 0 || is_intentional(catch, text) {
        return;
    }
    diagnostics.push(Diagnostic::new(Rule::EmptyCatch, body.start_byte(), "Empty catch block"));
}

fn is_intentional(catch: &Node, text: &str) -> bool {
    let parameter = catch.named_children(&mut catch.walk()).find(|c| c.kind() == "catch_formal_parameter");
    let name = parameter.and_then(|p| p.child_by_field_name("name"));
    name.is_some_and(|n| INTENTIONAL_NAMES.contains(&source(&n, text)))
}
