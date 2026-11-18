//! C005: a method, constructor, initializer or lambda with too many statements (Checkstyle
//! ExecutableStatementCount).
//!
//! Follows Checkstyle's count: for each statement list (a block or the statements of a `case` group) in the method
//! but outside nested methods and lambdas, half the number of nodes in Checkstyle's syntax tree of the list, rounded
//! down. An expression statement or a local variable declaration contributes its expression or each variable and a
//! separator, other statements one node, and a block its closing brace; so `{ if (a) {} if (b) {} }` counts 1.

use super::{is_constructor_call, is_method_like, token_before};
use crate::ast::{CaseKind, K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub struct ExecutableStatements {
    max: u32,
    /// The counts of the enclosing methods and lambdas; the first collects the code outside them.
    counts: Vec<u32>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ExecutableStatements {
    pub fn new(max: u32) -> ExecutableStatements {
        ExecutableStatements { max, counts: vec![0], diagnostics: Vec::new() }
    }

    pub fn enter(&mut self, tree: &Tree, ancestors: &[&Tree]) {
        if is_container(tree, ancestors) {
            self.counts.push(0);
        }
        let statements = match &tree.kind {
            // With the closing brace.
            K::Block { stmts, .. } => statement_nodes(stmts).div_ceil(2),
            // Checkstyle's group of `case 1: case 2: f();` holds the labels of both cases, whose statements are the
            // second's.
            K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
                cases.iter().filter(|c| c.kind == CaseKind::Statement).map(|c| statement_nodes(&c.stmts) / 2).sum()
            }
            _ => 0,
        };
        if let Some(count) = self.counts.last_mut() {
            *count += statements;
        }
    }

    pub fn leave(&mut self, tree: &Tree, ancestors: &[&Tree], context: &Context) {
        if !is_container(tree, ancestors) || self.counts.len() <= 1 {
            return;
        }
        let count = self.counts.pop().unwrap_or(0);
        if count > self.max {
            let position = match &tree.kind {
                K::Lambda { body, .. } => token_before(context, body.start, tree.start),
                _ => tree.start,
            };
            let message = format!("Executable statement count is {count} (max allowed is {}).", self.max);
            self.diagnostics.push(Diagnostic::new(Rule::ExecutableStatementCount, position, message));
        }
    }
}

fn is_container(tree: &Tree, ancestors: &[&Tree]) -> bool {
    matches!(tree.kind, K::Lambda { .. }) || is_method_like(tree, ancestors.last().copied())
}

/// The number of nodes that the statements have in Checkstyle's statement list.
fn statement_nodes(statements: &[Tree]) -> u32 {
    statements
        .iter()
        .map(|statement| match &statement.kind {
            K::ExprStmt(expression) if !is_constructor_call(expression) => 2,
            K::Vars(vars) => 2 * vars.len() as u32,
            _ => 1,
        })
        .sum()
}
