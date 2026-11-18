//! B003: empty `catch` block (Checkstyle EmptyCatchBlock, PMD EmptyCatchBlock, Error Prone EmptyCatch).
//!
//! A block with only `;` counts as empty. A comment in the block, or naming the exception like PMD's defaults
//! (`ignored`, `expected`) or `_`, documents that the exception is swallowed on purpose.

use crate::ast::{K, Tree};
use crate::lint::tree::{Context, has_comment};
use crate::lint::{Diagnostic, Rule};

const INTENTIONAL_NAMES: &[&str] = &["ignored", "expected", "_"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Try { catches, .. } = &tree.kind else { return };
    for catch in catches {
        let block = &catch.block;
        let K::Block { stmts, .. } = &block.kind else { continue };
        if stmts.iter().any(|s| !matches!(s.kind, K::Empty))
            || has_comment(context.text, block.start..block.end)
            || INTENTIONAL_NAMES.contains(&catch.param.name.as_str())
        {
            continue;
        }
        diagnostics.push(Diagnostic::new(Rule::EmptyCatch, block.start, "Empty catch block"));
    }
}
