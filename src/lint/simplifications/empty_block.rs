//! S017: blocks without statements (Checkstyle EmptyBlock with its default `statement` option and tokens, PMD
//! EmptyControlStatement) in `while`, `do`, `for`, `if`, `else`, `try`, `finally`, `switch`, `synchronized` and
//! initializers. A block with only a comment counts as empty, one with only `;` does not. Empty `catch` blocks are
//! B003's.

use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let blocks: Vec<&Tree> = match &tree.kind {
        K::If { then, else_, .. } => std::iter::once(&**then).chain(else_.as_deref()).collect(),
        K::While { body, .. }
        | K::DoWhile { body, .. }
        | K::For { body, .. }
        | K::ForEach { body, .. }
        | K::Synchronized { block: body, .. } => vec![body],
        K::Try { block, finally, .. } => std::iter::once(&**block).chain(finally.as_deref()).collect(),
        // Initializers.
        K::Class(class) => {
            let constant_bodies = class.enum_constants.iter().filter_map(|c| c.body.as_deref()).flatten();
            class.members.iter().chain(constant_bodies).collect()
        }
        K::NewClass { body: Some(members), .. } => members.iter().collect(),
        K::Switch { selector, cases } | K::SwitchExpr { selector, cases } => {
            if cases.is_empty()
                && let Some(open) = context.tokens().next(selector.end)
            {
                report(open.start, diagnostics);
            }
            return;
        }
        _ => return,
    };
    let tokens = context.tokens();
    for block in blocks {
        if let K::Block { is_static, stmts } = &block.kind
            && stmts.is_empty()
        {
            // A static initializer starts at `static`.
            let open = if *is_static { tokens.next(block.start).and_then(|k| tokens.next(k.end)) } else { None };
            report(open.map_or(block.start, |o| o.start), diagnostics);
        }
    }
}

fn report(position: usize, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic::new(Rule::EmptyBlock, position, "Must have at least one statement"));
}
