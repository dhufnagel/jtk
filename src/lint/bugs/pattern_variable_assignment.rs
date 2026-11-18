//! B031: assigning to a pattern variable, `if (o instanceof String s) { s = s.trim(); }` (Checkstyle
//! PatternVariableAssignment): the variable no longer holds the matched value, which readers expect.
//!
//! As in Checkstyle, this covers the variables of `instanceof` patterns (not of `case` patterns), searched in the
//! scope of the nearest enclosing `if`, `else` or `?:`: for an `if`, its condition and then branch plus the
//! expression, `return` and `if` statements directly after it; for an `else`, its body; for `?:`, the whole
//! expression. Assignments (also compound ones, `++` and `--`) to a variable of the same name in that scope are
//! reported, even if the name is shadowed.
//!
//! Deviations: like Checkstyle's current source, `++` and `--` count, which Checkstyle 14.4.0 does not report yet.
//! Checkstyle only finds one assignment per parent node, so of `c ? (s = a) : (s = b)` only the first; jtk reports
//! both.

use crate::ast::{CaseKind, K, Tree};
use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (cond, then, alternative) = match &tree.kind {
        K::Conditional { cond, then, else_ } => {
            check_scope(&[cond, then, else_], &[tree], diagnostics);
            return;
        }
        K::If { cond, then, else_ } => (cond, then, else_),
        _ => return,
    };
    let names = pattern_variables(&[cond, then]);
    if !names.is_empty() {
        let mut branches: Vec<&Tree> = vec![then];
        branches.extend(following_statements(tree, context));
        report_assignments(&branches, &names, diagnostics);
    }
    if let Some(alternative) = alternative {
        check_scope(&[alternative], &[alternative], diagnostics);
    }
}

fn check_scope(scope: &[&Tree], branches: &[&Tree], diagnostics: &mut Vec<Diagnostic>) {
    let names = pattern_variables(scope);
    if !names.is_empty() {
        report_assignments(branches, &names, diagnostics);
    }
}

/// Whether the tree starts a scope of its own.
fn is_scope(tree: &Tree) -> bool {
    matches!(tree.kind, K::If { .. } | K::Conditional { .. })
}

/// The names of the variables of the `instanceof` patterns in the trees (the parts of a scope), outside nested
/// scopes.
fn pattern_variables<'t>(trees: &[&'t Tree]) -> Vec<&'t str> {
    let mut names = Vec::new();
    for &tree in trees {
        if is_scope(tree) {
            continue;
        }
        let mut visit = |t: &'t Tree| {
            if let K::InstanceOf { target, .. } = &t.kind {
                pattern_names(target, &mut names);
            }
        };
        visit(tree);
        for_each_descendant(tree, &mut |t, _| {
            if is_scope(t) {
                return false;
            }
            visit(t);
            true
        });
    }
    names
}

/// The names of the variables of a pattern.
fn pattern_names<'t>(pattern: &'t Tree, names: &mut Vec<&'t str>) {
    match &pattern.kind {
        K::BindingPattern { var } => names.push(&var.name),
        K::RecordPattern { nested, .. } => nested.iter().for_each(|n| pattern_names(n, names)),
        _ => {}
    }
}

/// The expression, `return` and `if` statements directly after the statement, up to any other statement (also an
/// empty one, `;`). Like tree-sitter's expression statements, a `switch` statement counts.
fn following_statements<'t>(statement: &Tree, context: &Context<'t>) -> Vec<&'t Tree> {
    let siblings: &[Tree] = match context.parent().map(|p| &p.kind) {
        Some(K::Block { stmts, .. }) => stmts,
        Some(K::Switch { cases, .. } | K::SwitchExpr { cases, .. }) => cases
            .iter()
            .filter(|c| c.kind == CaseKind::Statement)
            .map(|c| &c.stmts[..])
            .find(|stmts| stmts.iter().any(|s| std::ptr::eq(s, statement)))
            .unwrap_or_default(),
        _ => &[],
    };
    siblings
        .iter()
        .skip_while(|s| !std::ptr::eq(*s, statement))
        .skip(1)
        .take_while(|s| matches!(s.kind, K::ExprStmt(_) | K::Switch { .. } | K::Return(_) | K::If { .. }))
        .collect()
}

fn report_assignments(branches: &[&Tree], names: &[&str], diagnostics: &mut Vec<Diagnostic>) {
    for branch in branches {
        let mut visit = |tree: &Tree| {
            let target = match &tree.kind {
                K::Assign { lhs, .. } => lhs,
                K::Unary { op, expr, .. } if op == "++" || op == "--" => expr,
                _ => return,
            };
            if let K::Ident(name) = &target.kind
                && names.contains(&name.as_str())
            {
                diagnostics.push(Diagnostic::new(
                    Rule::PatternVariableAssignment,
                    target.start,
                    format!("Assignment of pattern variable '{name}' is not allowed"),
                ));
            }
        };
        visit(branch);
        for_each_descendant(branch, &mut |t, _| {
            visit(t);
            true
        });
    }
}
