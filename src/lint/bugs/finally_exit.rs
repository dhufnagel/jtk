//! B010: `return` or `throw` in a `finally` block (PMD ReturnFromFinallyBlock, DoNotThrowExceptionInFinally):
//! discards the exception or result of the `try` block.

use tree_sitter::Node;

use crate::lint::syntax::{NESTED_CODE, for_each_descendant};
use crate::lint::{Diagnostic, Rule};

pub fn check(finally: &Node, _text: &str, diagnostics: &mut Vec<Diagnostic>) {
    for_each_descendant(finally, NESTED_CODE, |node| {
        let message = match node.kind() {
            "return_statement" => "return in finally discards the exception or result of the try block",
            "throw_statement" => "throw in finally discards the exception or result of the try block",
            _ => return,
        };
        diagnostics.push(Diagnostic::new(Rule::FinallyExit, node.start_byte(), message));
    });
}
