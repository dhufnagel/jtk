//! Nesting depths, as in Checkstyle across the whole file (a statement in a lambda or local class within another one
//! counts as nested):
//!
//! - C014: `if` statements (Checkstyle NestedIfDepth, PMD AvoidDeeplyNestedIfStmts); an `else if`, also written as an
//!   `else` block holding only the `if`, does not nest,
//! - C015: `for` loops (Checkstyle NestedForDepth),
//! - C016: `try` statements (Checkstyle NestedTryDepth).

use crate::ast::{K, Tree};
use crate::lint::{Diagnostic, Rule};

/// Reports the statements that `nests` accepts nested in more than `max` others.
pub struct NestingDepth {
    rule: Rule,
    /// The name of the statement in the message.
    name: &'static str,
    max: usize,
    /// Whether the tree (with these ancestors) is a statement that nests, rather than one of another kind or one that
    /// continues its parent (an `else if` continues the chain of its `if` rather than nesting in it).
    nests: fn(&Tree, &[&Tree]) -> bool,
    depth: usize,
    pub diagnostics: Vec<Diagnostic>,
}

impl NestingDepth {
    pub fn nested_if(max: usize) -> NestingDepth {
        let nests =
            |tree: &Tree, ancestors: &[&Tree]| matches!(tree.kind, K::If { .. }) && !is_else_if(tree, ancestors);
        NestingDepth::new(Rule::NestedIfDepth, "if-else", max, nests)
    }

    pub fn nested_for(max: usize) -> NestingDepth {
        NestingDepth::new(Rule::NestedForDepth, "for", max, |tree, _| {
            matches!(tree.kind, K::For { .. } | K::ForEach { .. })
        })
    }

    pub fn nested_try(max: usize) -> NestingDepth {
        NestingDepth::new(Rule::NestedTryDepth, "try", max, |tree, _| matches!(tree.kind, K::Try { .. }))
    }

    fn new(rule: Rule, name: &'static str, max: usize, nests: fn(&Tree, &[&Tree]) -> bool) -> NestingDepth {
        NestingDepth { rule, name, max, nests, depth: 0, diagnostics: Vec::new() }
    }

    pub fn enter(&mut self, tree: &Tree, ancestors: &[&Tree]) {
        if !(self.nests)(tree, ancestors) {
            return;
        }
        if self.depth > self.max {
            let message = format!("Nested {} depth is {} (max allowed is {}).", self.name, self.depth, self.max);
            self.diagnostics.push(Diagnostic::new(self.rule, tree.start, message));
        }
        self.depth += 1;
    }

    pub fn leave(&mut self, tree: &Tree, ancestors: &[&Tree]) {
        if (self.nests)(tree, ancestors) {
            self.depth -= 1;
        }
    }
}

/// Whether the `if` is the alternative of another one, directly or as the only statement of an `else` block.
fn is_else_if(tree: &Tree, ancestors: &[&Tree]) -> bool {
    let alternative_of_if = |alternative: &Tree, parent: Option<&&Tree>| {
        parent.is_some_and(|p| matches!(&p.kind, K::If { else_: Some(e), .. } if std::ptr::eq(&**e, alternative)))
    };
    if let Some((parent, rest)) = ancestors.split_last()
        && let K::Block { stmts, .. } = &parent.kind
    {
        return matches!(stmts.as_slice(), [only] if std::ptr::eq(only, tree))
            && alternative_of_if(parent, rest.last());
    }
    alternative_of_if(tree, ancestors.last())
}
