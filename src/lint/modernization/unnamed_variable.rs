//! M008: unused `catch` parameter, lambda parameter or `try` resource that can be the unnamed variable `_`
//! (Checkstyle UnusedCatchParameterShouldBeUnnamed, UnusedLambdaParameterShouldBeUnnamed,
//! UnusedTryResourceShouldBeUnnamed), Java 22.
//!
//! As in Checkstyle, a variable that is only assigned (`e = null`) is unused; the fix (renaming the declaration to
//! `_`) is only offered when the name does not occur at all.

use tree_sitter::Node;

use crate::lint::syntax::{is_name_reference_in, named_children, operator, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn catch_parameter(clause: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(parameter) = named_children(clause).into_iter().find(|c| c.kind() == "catch_formal_parameter") else {
        return;
    };
    let (Some(name), Some(body)) = (parameter.child_by_field_name("name"), clause.child_by_field_name("body")) else {
        return;
    };
    report(&name, parameter.start_byte(), &[body], "catch parameter", text, diagnostics);
}

pub fn lambda_parameter(lambda: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (Some(parameters), Some(body)) = (lambda.child_by_field_name("parameters"), lambda.child_by_field_name("body"))
    else {
        return;
    };
    let names = match parameters.kind() {
        "identifier" => vec![parameters],
        "inferred_parameters" => named_children(&parameters).into_iter().filter(|c| c.kind() == "identifier").collect(),
        _ => named_children(&parameters).iter().filter_map(parameter_name).collect(),
    };
    for name in names {
        report(&name, name.start_byte(), &[body], "lambda parameter", text, diagnostics);
    }
}

pub fn try_resource(statement: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (Some(resources), Some(body)) =
        (statement.child_by_field_name("resources"), statement.child_by_field_name("body"))
    else {
        return;
    };
    for resource in named_children(&resources).into_iter().filter(|r| r.kind() == "resource") {
        if resource.child_by_field_name("type").is_none() {
            continue;
        }
        if let Some(name) = resource.child_by_field_name("name") {
            report(&name, name.start_byte(), &[resources, body], "try resource", text, diagnostics);
        }
    }
}

fn parameter_name<'t>(parameter: &Node<'t>) -> Option<Node<'t>> {
    match parameter.kind() {
        "formal_parameter" => parameter.child_by_field_name("name"),
        "spread_parameter" => named_children(parameter)
            .into_iter()
            .find(|c| c.kind() == "variable_declarator")
            .and_then(|d| d.child_by_field_name("name")),
        _ => None,
    }
}

fn report(name: &Node, offset: usize, scopes: &[Node], what: &str, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let variable = source(name, text);
    if name.kind() != "identifier" || variable == "_" {
        return;
    }
    let (read, mentioned) = usage(scopes, variable, text);
    if !read {
        let message = format!("Unused {what} '{variable}' should be unnamed");
        let mut diagnostic = Diagnostic::new(Rule::UnnamedVariable, offset, message);
        if !mentioned {
            diagnostic = diagnostic.with_fix(vec![Edit::new(name.byte_range(), "_")]);
        }
        diagnostics.push(diagnostic);
    }
}

/// Whether the name is read in the scopes, and whether it occurs there at all. Like Checkstyle, any identifier with
/// the name counts, except member names (`a.name`, `name()`) and the target of a plain assignment. Nested `catch`
/// parameters and lambda parameters of the same name hide the variable.
fn usage(scopes: &[Node], variable: &str, text: &str) -> (bool, bool) {
    let mut read = false;
    let mut mentioned = false;
    let mut pending: Vec<(Node, Option<Node>)> = scopes.iter().map(|s| (*s, None)).collect();
    while let Some((node, parent)) = pending.pop() {
        if node.kind() == "identifier" && source(&node, text) == variable {
            if parent.is_none_or(|p| is_name_reference_in(&node, &p)) {
                mentioned = true;
                let is_assigned = parent.is_some_and(|p| {
                    p.kind() == "assignment_expression"
                        && p.child_by_field_name("left") == Some(node)
                        && operator(&p, text) == "="
                });
                read |= !is_assigned;
            }
            continue;
        }
        if declares(&node, variable, text) {
            continue;
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor).map(|c| (c, Some(node))));
    }
    (read, mentioned)
}

/// Whether a nested `catch` clause or lambda declares a parameter with the name.
fn declares(node: &Node, variable: &str, text: &str) -> bool {
    let names = match node.kind() {
        "catch_clause" => named_children(node)
            .into_iter()
            .find(|c| c.kind() == "catch_formal_parameter")
            .and_then(|p| p.child_by_field_name("name"))
            .into_iter()
            .collect(),
        "lambda_expression" => match node.child_by_field_name("parameters") {
            Some(p) if p.kind() == "identifier" => vec![p],
            Some(p) if p.kind() == "inferred_parameters" => named_children(&p),
            Some(p) => named_children(&p).iter().filter_map(parameter_name).collect(),
            None => Vec::new(),
        },
        _ => Vec::new(),
    };
    names.iter().any(|n| source(n, text) == variable)
}
