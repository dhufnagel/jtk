//! C001: a method with too many decision points (Checkstyle CyclomaticComplexity, PMD CyclomaticComplexity).
//!
//! As in Checkstyle (with `switchBlockAsSingleDecisionPoint=false`), the complexity of a method, constructor or
//! initializer is 1 plus the number of `if`, `while`, `do`, `for`, `case` (one per `case` keyword), `catch`, `?:`,
//! `&&`, `||` and `when`. Code in lambdas counts for the enclosing method, code in methods of local and anonymous
//! classes only for those.

use super::is_method_like;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

pub struct Cyclomatic {
    max: u64,
    /// The complexity of each enclosing method; the first entry collects the code outside methods.
    stack: Vec<u64>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Cyclomatic {
    pub fn new(max: u64) -> Cyclomatic {
        Cyclomatic { max, stack: vec![1], diagnostics: Vec::new() }
    }

    pub fn enter(&mut self, tree: &Tree, ancestors: &[&Tree]) {
        if is_method_like(tree, ancestors.last().copied()) {
            self.stack.push(1);
        } else if let Some(top) = self.stack.last_mut() {
            *top += decision_points(tree);
        }
    }

    pub fn leave(&mut self, tree: &Tree, ancestors: &[&Tree]) {
        if is_method_like(tree, ancestors.last().copied()) {
            let complexity = self.stack.pop().unwrap_or(1);
            if complexity > self.max {
                let message = format!("Cyclomatic Complexity is {complexity} (max allowed is {}).", self.max);
                self.diagnostics.push(Diagnostic::new(Rule::CyclomaticComplexity, tree.start, message));
            }
        }
    }
}

/// The decision points of the tree itself (with its `catch` clauses and `case` labels).
fn decision_points(tree: &Tree) -> u64 {
    match &tree.kind {
        K::If { .. } | K::While { .. } | K::DoWhile { .. } | K::For { .. } | K::ForEach { .. } => 1,
        K::Conditional { .. } => 1,
        K::Binary { op, .. } => u64::from(op == "&&" || op == "||"),
        K::Try { catches, .. } => catches.len() as u64,
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            cases.iter().map(|c| u64::from(!c.is_default) + u64::from(c.guard.is_some())).sum()
        }
        _ => 0,
    }
}
