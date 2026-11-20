//! C004: a method, class, record or file with too many non-commenting source statements (Checkstyle JavaNCSS, PMD
//! NcssCount).
//!
//! As in Checkstyle, counts package and (non-static) import declarations, class, interface and record declarations, methods,
//! constructors and initializers, field declarations and local variable declarations (one per statement),
//! expression statements (not the expression body of a switch rule), explicit constructor calls, `if`, `else`,
//! loops, `switch`, `case` and `default` labels (also the `default` of interface methods and annotation elements),
//! `break`, `continue`, `return`, `throw`, `synchronized`, `catch`, `finally` and labeled statements. Each count goes
//! to the enclosing method, class (not interface or enum), record and the file.

use super::{file_position, is_annotation_type, is_method_like};
use crate::lint::options::JavaNcss;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Modifier, Tree};

pub struct Ncss {
    limits: JavaNcss,
    /// The counts of the enclosing methods, classes and records; the first is the file's.
    counters: Vec<u32>,
    diagnostics: Vec<Diagnostic>,
}

impl Ncss {
    pub fn new(limits: JavaNcss) -> Ncss {
        Ncss { limits, counters: vec![0], diagnostics: Vec::new() }
    }

    pub fn enter(&mut self, tree: &Tree, ancestors: &[&Tree], context: &Context) {
        let parent = ancestors.last().copied();
        if is_counter(tree, parent) {
            self.counters.push(0);
        }
        let count = statements(tree, parent, context);
        if count > 0 {
            for counter in &mut self.counters {
                *counter += count;
            }
        }
    }

    pub fn leave(&mut self, tree: &Tree, ancestors: &[&Tree]) {
        if !is_counter(tree, ancestors.last().copied()) || self.counters.len() <= 1 {
            return;
        }
        let count = self.counters.pop().unwrap_or(0);
        let (what, max) = match &tree.kind {
            K::Class(c) if c.kind == ClassKind::Class => ("class", self.limits.class_maximum),
            K::Class(_) => ("record", self.limits.record_maximum),
            _ => ("method", self.limits.method_maximum),
        };
        if count > max {
            let message = format!("NCSS for this {what} is {count} (max allowed is {max}).");
            self.diagnostics.push(Diagnostic::new(Rule::JavaNcss, tree.start, message));
        }
    }

    /// The violations, with the file's.
    pub fn finish(mut self, context: &Context) -> Vec<Diagnostic> {
        let count = self.counters[0];
        let max = self.limits.file_maximum;
        if count > max {
            let message = format!("NCSS for this file is {count} (max allowed is {max}).");
            self.diagnostics.push(Diagnostic::new(Rule::JavaNcss, file_position(context), message));
        }
        self.diagnostics
    }
}

/// Whether the tree has a count of its own: a method, class or record.
fn is_counter(tree: &Tree, parent: Option<&Tree>) -> bool {
    match &tree.kind {
        K::Class(c) => matches!(c.kind, ClassKind::Class | ClassKind::Record),
        _ => is_method_like(tree, parent),
    }
}

/// The number of statements the tree counts as, with its own keywords such as `else` or `case`.
fn statements(tree: &Tree, parent: Option<&Tree>, context: &Context) -> u32 {
    // Not the initialization of a `for` loop, nor a variable of a compact source file (which tree-sitter parses as
    // a local variable outside a block).
    let is_statement = || match parent.map(|p| &p.kind) {
        Some(K::For { init, .. }) => !init.iter().any(|i| std::ptr::eq(i, tree)),
        Some(K::CompilationUnit { .. }) => false,
        _ => true,
    };
    match &tree.kind {
        K::CompilationUnit { package, .. } => u32::from(package.is_some()),
        K::Import { is_static, .. } => u32::from(!is_static),
        K::Class(c) => u32::from(matches!(c.kind, ClassKind::Class | ClassKind::Interface | ClassKind::Record)),
        // An element of an annotation type counts only its `default` value.
        K::Method(m) if parent.is_some_and(is_annotation_type) => u32::from(m.default_value.is_some()),
        K::Method(m) => {
            // The text is searched first, which is cheaper than lexing the file.
            let is_default = context.text[tree.start..m.name_pos].contains("default") && m.mods.has(Modifier::Default);
            1 + u32::from(is_default)
        }
        K::Block { .. } => u32::from(is_method_like(tree, parent)),
        K::If { else_, .. } => 1 + u32::from(else_.is_some()),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            let labels = cases.iter().map(|c| {
                let has_default = c.is_default || c.labels.iter().any(|l| matches!(l.kind, K::DefaultCaseLabel));
                u32::from(!c.is_default) + u32::from(has_default)
            });
            1 + labels.sum::<u32>()
        }
        K::Try { catches, finally, .. } => catches.len() as u32 + u32::from(finally.is_some()),
        K::While { .. }
        | K::DoWhile { .. }
        | K::For { .. }
        | K::ForEach { .. }
        | K::Break(_)
        | K::Continue(_)
        | K::Return(_)
        | K::Throw(_)
        | K::Synchronized { .. }
        | K::Labeled { .. } => 1,
        K::Vars(_) | K::ExprStmt(_) => u32::from(is_statement()),
        _ => 0,
    }
}
