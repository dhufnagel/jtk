//! S014: C-style array brackets after a variable name or method parameter list, `int a[]` instead of `int[] a`
//! (Checkstyle ArrayTypeStyle with the default Java style). Like Checkstyle, brackets on a later line than the name
//! are only reported if they start right of the name's column.
//!
//! The fix moves the brackets to the type, unless the declaration declares several variables (whose types would
//! change too) or the brackets are annotated or contain comments.

use std::ops::Range;

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree, VarDecl};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    match &tree.kind {
        K::Vars(vars) => {
            for var in vars {
                check_variable(var, vars.len() == 1, context, diagnostics);
            }
        }
        K::Method(method) => {
            for parameter in method.params.iter().filter(|p| !is_variable_arity(p, context)) {
                check_variable(parameter, true, context, diagnostics);
            }
            if let Some(ret) = method.ret.as_deref().filter(|r| r.end > method.name_pos) {
                // The dimensions follow the parameters' `)`.
                let close = match method.params.last().or(method.receiver.as_deref()) {
                    Some(last) => context.tokens().next(last.end),
                    None => context.tokens().in_range(method.name_pos..ret.end).nth(2),
                };
                if let Some(close) = close {
                    let name = method.name_pos..method.name_pos + method.name.len();
                    report(name, close.end..ret.end, true, context, diagnostics);
                }
            }
        }
        K::Class(class) => {
            for component in class.record_components.iter().filter(|p| !is_variable_arity(p, context)) {
                check_variable(component, true, context, diagnostics);
            }
        }
        K::Lambda { params, .. } => {
            for parameter in params.iter().filter(|p| !is_variable_arity(p, context)) {
                check_variable(parameter, true, context, diagnostics);
            }
        }
        K::Try { resources, catches, .. } => {
            for resource in resources {
                if let K::Var(var) = &resource.kind {
                    check_variable(var, true, context, diagnostics);
                }
            }
            for catch in catches {
                // tree-sitter's catch parameter has no type field, which the fix needs.
                check_variable(&catch.param, false, context, diagnostics);
            }
        }
        K::ForEach { var, .. } => check_variable(var, true, context, diagnostics),
        _ => {}
    }
}

/// `String... names`, which tree-sitter does not check.
fn is_variable_arity(parameter: &VarDecl, context: &Context) -> bool {
    context.tokens().previous(parameter.name_pos).is_some_and(|t| &context.text[t] == "...")
}

fn check_variable(var: &VarDecl, fixable: bool, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let name = var.name_pos..var.name_pos + var.name.len();
    if let Some(ty) = var.ty.as_deref().filter(|t| t.end > name.end) {
        report(name.clone(), name.end..ty.end, fixable, context, diagnostics);
    }
}

/// Reports the brackets in `dimensions` (which may start with whitespace) after the `name`.
fn report(
    name: Range<usize>,
    dimensions: Range<usize>,
    fixable: bool,
    context: &Context,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (text, tokens) = (context.text, context.tokens());
    let Some(first) = tokens.next(dimensions.start) else { return };
    let dimensions = first.start..dimensions.end;
    let brackets: Vec<Range<usize>> = tokens.in_range(dimensions.clone()).filter(|t| &text[t.clone()] == "[").collect();
    let annotated = tokens.in_range(dimensions.clone()).any(|t| &text[t] == "@");
    let fix = (fixable && !annotated && !tokens.has_comment(dimensions.clone()))
        .then(|| tokens.previous(name.start))
        .flatten()
        .map(|declared_type| {
            let whitespace = text[..dimensions.start].len() - text[..dimensions.start].trim_end().len();
            vec![
                Edit::new(declared_type.end..declared_type.end, "[]".repeat(brackets.len())),
                Edit::new(dimensions.start - whitespace..dimensions.end, ""),
            ]
        });
    // Like Checkstyle, brackets on a later line than the name count as following the type if they start left of it.
    let (name_line, name_column) = line_and_column(name.start, text);
    for bracket in brackets {
        let (line, column) = line_and_column(bracket.start, text);
        if line != name_line && column <= name_column {
            continue;
        }
        let mut diagnostic =
            Diagnostic::new(Rule::ArrayTypeStyle, bracket.start, "Array brackets should follow the type");
        if let Some(fix) = &fix {
            diagnostic = diagnostic.with_fix(fix.clone());
        }
        diagnostics.push(diagnostic);
    }
}

/// The start of the offset's line, and its column in characters.
fn line_and_column(offset: usize, text: &str) -> (usize, usize) {
    let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
    (line_start, text[line_start..offset].chars().count())
}
