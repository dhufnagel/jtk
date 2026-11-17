//! S017: blocks without statements (Checkstyle EmptyBlock with its default `statement` option and tokens, PMD
//! EmptyControlStatement) in `while`, `do`, `for`, `if`, `else`, `try`, `finally`, `switch`, `synchronized` and
//! initializers. A block with only a comment counts as empty, one with only `;` does not. Empty `catch` blocks are
//! B003's.

use tree_sitter::Node;

use crate::lint::syntax::{is_comment, named_children};
use crate::lint::{Diagnostic, Rule};

pub fn check(node: &Node, _text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let blocks: Vec<Node> = match node.kind() {
        "if_statement" => ["consequence", "alternative"].iter().filter_map(|f| node.child_by_field_name(f)).collect(),
        "finally_clause" | "static_initializer" => named_children(node),
        // Instance initializers.
        "class_body" | "enum_body_declarations" => named_children(node),
        "switch_expression" => {
            let Some(body) = node.child_by_field_name("body") else { return };
            if named_children(&body).iter().all(is_comment) {
                report(&body, diagnostics);
            }
            return;
        }
        _ => node.child_by_field_name("body").into_iter().collect(),
    };
    for block in blocks.iter().filter(|b| b.kind() == "block") {
        let is_empty = block.children(&mut block.walk()).all(|c| matches!(c.kind(), "{" | "}") || is_comment(&c));
        if is_empty {
            report(block, diagnostics);
        }
    }
}

fn report(block: &Node, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic::new(Rule::EmptyBlock, block.start_byte(), "Must have at least one statement"));
}
