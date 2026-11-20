//! A014: a local variable declared more than `allowed-distance` (3) statements before its first use (Checkstyle
//! VariableDeclarationUsageDistance with its default properties: `ignoreFinal`, without `validateBetweenScopes`).
//!
//! As in Checkstyle, the distance counts the statements of the same block between the declaration and the statement
//! that uses the variable (other declarations do not count; a use in a nested block, also a labeled or
//! `synchronized` one, has distance 0, a use in a loop, `if` or `try` one more), a use is any identifier with the
//! variable's name, and a variable initialized before a sequence of calls on one object (`a.setX(); a.setY(); ...
//! use(variable)`) is not reported.

use std::collections::HashSet;
use std::ops::Range;

use super::has_keyword;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{CaseKind, K, Tree};

/// Checks the variables declared in a block or `case` group.
pub fn check<'t>(tree: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    match &tree.kind {
        K::Block { stmts, .. } => check_statements(stmts, context, diagnostics),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            for case in cases.iter().filter(|c| c.kind == CaseKind::Statement) {
                check_statements(&case.stmts, context, diagnostics);
            }
        }
        _ => {}
    }
}

fn check_statements<'t>(statements: &'t [Tree], context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    if !statements.iter().any(|s| matches!(s.kind, K::Vars(_))) {
        return;
    }
    let statements: Vec<&Tree> = statements.iter().filter(|s| !matches!(s.kind, K::Empty)).collect();
    // Computed lazily: most blocks declare no variable that needs them.
    let mut names_used: Option<Vec<HashSet<&str>>> = None;
    let allowed = context.options.declaration_usage_distance.allowed_distance;
    for (index, declaration) in statements.iter().enumerate() {
        let K::Vars(vars) = &declaration.kind else { continue };
        if has_keyword(&vars[0].mods, vars[0].name_pos, "final", context) {
            continue;
        }
        let names_used =
            names_used.get_or_insert_with(|| statements.iter().map(|s| identifiers(s.start..s.end, context)).collect());
        for (position, var) in vars.iter().enumerate() {
            let name = var.name.as_str();
            if vars[position + 1..].iter().any(|v| contains_identifier(v.name_pos..v.end, name, context)) {
                continue;
            }
            let later = statements[index + 1..].iter().copied().zip(&names_used[index + 1..]);
            // A sequence of calls may follow only the first variable of a declaration (see `usage_distance`), so
            // later variables of the declaration interrupt it.
            let is_last = position + 1 == vars.len();
            let distance = usage_distance(later, name, is_last, context);
            if distance > allowed {
                let message = format!(
                    "Distance between variable '{name}' declaration and its first usage is {distance}, but allowed \
                     {allowed}"
                );
                diagnostics.push(Diagnostic::new(Rule::DeclarationUsageDistance, declaration.start, message));
            }
        }
    }
}

/// The number of statements up to and including the first that uses the variable, without declarations; 0 if the
/// variable is unused or first used in a nested block, which is another scope. 1 if the statements before it are
/// calls on one object, as when the object is initialized, and the first use is in another such call or a
/// declaration. As in Checkstyle, a declaration of several variables only belongs to such a sequence up to its first
/// variable.
fn usage_distance<'a>(
    later: impl Iterator<Item = (&'a Tree, &'a HashSet<&'a str>)>,
    name: &str,
    sequence_possible: bool,
    context: &Context,
) -> u32 {
    let mut distance = 0;
    // The object of all calls so far, while the statements form a sequence of calls on one object.
    let mut sequence = sequence_possible.then_some(None::<String>);
    for (statement, names) in later {
        let declaration = match &statement.kind {
            K::Vars(vars) => Some(vars),
            _ => None,
        };
        if !names.contains(name) {
            if let Some(vars) = declaration {
                if vars.len() > 1 {
                    sequence = None;
                }
                continue;
            }
            distance += 1;
            sequence = match (sequence, call_object(statement, context.text)) {
                (Some(None), Some(object)) => Some(Some(object)),
                (Some(Some(current)), Some(object)) if current == object => Some(Some(current)),
                _ => None,
            };
            continue;
        }
        if is_nested_block(statement) {
            return 0;
        }
        let continues_sequence = match &sequence {
            Some(current) => {
                // Whether the declaration uses the variable before its second variable.
                declaration.is_some_and(|vars| contains_identifier(statement.start..vars[0].end, name, context))
                    || call_object(statement, context.text)
                        .is_some_and(|object| current.as_ref().is_none_or(|c| *c == object))
            }
            None => false,
        };
        return if continues_sequence { 1 } else { distance + 1 };
    }
    0
}

/// Whether the statement is a block, possibly labeled or `synchronized`.
fn is_nested_block(statement: &Tree) -> bool {
    match &statement.kind {
        K::Block { .. } | K::Synchronized { .. } => true,
        K::Labeled { body, .. } => is_nested_block(body),
        _ => false,
    }
}

/// The object (without whitespace) of a statement `object.method(...);`.
fn call_object(statement: &Tree, text: &str) -> Option<String> {
    let K::ExprStmt(call) = &statement.kind else { return None };
    let K::MethodInvocation { select, .. } = &call.kind else { return None };
    let K::MemberSelect { expr: object, name } = &select.kind else { return None };
    if name == "super" || name == "this" {
        // `outer.super(...)`, a constructor call.
        return None;
    }
    // In `Outer.super.method()`, the object is `Outer`.
    let object = match &object.kind {
        K::MemberSelect { expr, name } if name == "super" => expr,
        _ => object,
    };
    Some(text[object.start..object.end].split_whitespace().collect())
}

/// The names in a range of the source.
fn identifiers<'t>(range: Range<usize>, context: &Context<'t>) -> HashSet<&'t str> {
    let text = context.text;
    context
        .tokens()
        .in_range(range)
        .map(|token| &text[token])
        .filter(|token| token.starts_with(|c: char| c.is_alphabetic() || c == '_' || c == '$'))
        .collect()
}

fn contains_identifier(range: Range<usize>, name: &str, context: &Context) -> bool {
    context.tokens().in_range(range).any(|token| &context.text[token] == name)
}
