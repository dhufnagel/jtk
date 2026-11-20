//! B010: `return` or `throw` in a `finally` block (PMD ReturnFromFinallyBlock, DoNotThrowExceptionInFinally):
//! discards the exception or result of the `try` block.

use super::shared_a::for_each_in_same_code;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, _context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Try { finally: Some(finally), .. } = &tree.kind else { return };
    for_each_in_same_code(finally, &mut |statement| {
        let message = match statement.kind {
            K::Return(_) => "return in finally discards the exception or result of the try block",
            K::Throw(_) => "throw in finally discards the exception or result of the try block",
            _ => return,
        };
        diagnostics.push(Diagnostic::new(Rule::FinallyExit, statement.start, message));
    });
}
