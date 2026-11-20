//! M008: unused `catch` parameter, lambda parameter or `try` resource that can be the unnamed variable `_`
//! (Checkstyle UnusedCatchParameterShouldBeUnnamed, UnusedLambdaParameterShouldBeUnnamed,
//! UnusedTryResourceShouldBeUnnamed), Java 22.
//!
//! As in Checkstyle, a variable that is only assigned (`e = null`) is unused; the fix (renaming the declaration to
//! `_`) is only offered when the name does not occur at all.

use crate::lint::tree::{Context, is_type_child};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree, VarDecl};

use super::shared_b::is_variable_name;

/// Checks the catch parameters and resources of a `try`.
pub fn check_try(statement: &Tree, _context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Try { resources, block, catches, .. } = &statement.kind else { return };
    for catch in catches {
        report(&catch.param, catch.param.start, &[&catch.block], "catch parameter", diagnostics);
    }
    let mut scopes: Vec<&Tree> = resources.iter().collect();
    scopes.push(block);
    for resource in resources {
        if let K::Var(var) = &resource.kind {
            report(var, var.name_pos, &scopes, "try resource", diagnostics);
        }
    }
}

pub fn check_lambda(lambda: &Tree, _context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Lambda { params, body } = &lambda.kind else { return };
    for param in params {
        report(param, param.name_pos, &[body], "lambda parameter", diagnostics);
    }
}

fn report(var: &VarDecl, offset: usize, scopes: &[&Tree], what: &str, diagnostics: &mut Vec<Diagnostic>) {
    let variable = var.name.as_str();
    if variable == "_" {
        return;
    }
    let mut usage = Usage::default();
    for scope in scopes {
        usage.visit(scope, None, variable);
    }
    if !usage.read {
        let message = format!("Unused {what} '{variable}' should be unnamed");
        let mut diagnostic = Diagnostic::new(Rule::UnnamedVariable, offset, message);
        if !usage.mentioned {
            let name_end = var.name_pos + variable.len();
            diagnostic = diagnostic.with_fix(vec![Edit::new(var.name_pos..name_end, "_")]);
        }
        diagnostics.push(diagnostic);
    }
}

/// Whether the name is read in the scopes, and whether it occurs there at all. Like Checkstyle, any variable name
/// counts, except the target of a plain assignment. Nested `catch` parameters and lambda parameters of the same name
/// hide the variable.
#[derive(Default)]
struct Usage {
    read: bool,
    mentioned: bool,
}

impl Usage {
    fn visit(&mut self, tree: &Tree, parent: Option<&Tree>, variable: &str) {
        match &tree.kind {
            K::Ident(name) => {
                if name == variable && parent.is_none_or(|p| is_variable_name(tree, p)) {
                    self.mentioned = true;
                    let is_assigned = parent.is_some_and(
                        |p| matches!(&p.kind, K::Assign { op, lhs, .. } if op == "=" && std::ptr::eq(&**lhs, tree)),
                    );
                    self.read |= !is_assigned;
                }
                return;
            }
            K::Lambda { params, .. } if params.iter().any(|p| p.name == variable) => return,
            K::Try { resources, block, catches, finally } => {
                for child in resources.iter().chain([&**block]) {
                    self.visit(child, Some(tree), variable);
                }
                for catch in catches.iter().filter(|c| c.param.name != variable) {
                    self.visit(&catch.block, Some(tree), variable);
                }
                if let Some(finally) = finally {
                    self.visit(finally, Some(tree), variable);
                }
                return;
            }
            K::Annotation { args, .. } => {
                for arg in args {
                    match &arg.kind {
                        K::Assign { rhs, .. } => self.visit(rhs, Some(arg), variable),
                        _ => self.visit(arg, Some(tree), variable),
                    }
                }
                return;
            }
            _ => {}
        }
        tree.for_each_child(&mut |child| {
            if !is_type_child(tree, child) {
                self.visit(child, Some(tree), variable);
            }
        });
    }
}
