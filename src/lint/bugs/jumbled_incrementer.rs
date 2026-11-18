//! B015: a nested `for` loop that increments the outer loop's variable (PMD JumbledIncrementer).

use super::modified_control_variable::loop_variables;
use super::shared_a::modified_name;
use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::For { update, .. } = &tree.kind else { return };
    let updated: Vec<&str> =
        update.iter().filter_map(modified_name).filter(|v| !loop_variables(tree).any(|own| own == *v)).collect();
    if updated.is_empty() {
        return;
    }
    // The enclosing loops in the same code: up to a lambda or class body.
    let ancestors = &context.ancestors;
    for (index, outer) in ancestors.iter().enumerate().rev() {
        let child = ancestors.get(index + 1).copied().unwrap_or(tree);
        let in_class_body = match &outer.kind {
            K::Lambda { .. } | K::Class(_) => true,
            K::NewClass { body: Some(body), .. } => body.iter().any(|m| std::ptr::eq(m, child)),
            _ => false,
        };
        if in_class_body {
            return;
        }
        if let Some(variable) = updated.iter().find(|v| loop_variables(outer).any(|o| o == **v)) {
            let message = format!("Inner loop updates the outer loop's variable '{variable}'");
            diagnostics.push(Diagnostic::new(Rule::JumbledIncrementer, tree.start, message));
            return;
        }
    }
}
