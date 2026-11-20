//! B014: `for` loop variable modified in the loop body (Checkstyle ModifiedControlVariable, PMD
//! AvoidReassigningLoopVariables).

use super::shared_a::{for_each_in_same_code, modified_name};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, _context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (variables, body) = match &tree.kind {
        K::ForEach { var, body, .. } => (vec![var.name.as_str()], body),
        // Like Checkstyle, a variable only updated in the body is not a control variable.
        K::For { update, body, .. } => {
            let updated: Vec<&str> = update.iter().filter_map(modified_name).collect();
            (loop_variables(tree).filter(|v| updated.contains(v)).collect(), body)
        }
        _ => return,
    };
    if variables.is_empty() {
        return;
    }
    let mut check_tree = |tree: &Tree| {
        if let Some(name) = modified_name(tree)
            && variables.contains(&name)
        {
            let message = format!("Loop variable '{name}' is modified in the loop body");
            diagnostics.push(Diagnostic::new(Rule::ModifiedControlVariable, tree.start, message));
        }
    };
    for_each_in_same_code(body, &mut check_tree);
}

/// The variables declared in the initializer of a `for` loop.
pub(super) fn loop_variables(tree: &Tree) -> impl Iterator<Item = &str> {
    let init = match &tree.kind {
        K::For { init, .. } => init.as_slice(),
        _ => &[],
    };
    init.iter()
        .filter_map(|i| match &i.kind {
            K::Vars(vars) => Some(vars),
            _ => None,
        })
        .flatten()
        .map(|v| v.name.as_str())
}
