//! M003: a pattern `case ... ->` whose block only holds an `if` without `else` (Checkstyle WhenShouldBeUsed),
//! Java 21: the condition can be a `when` guard. As in Checkstyle, the block may also hold `break`, empty statements
//! and `if` statements with `else`, as long as exactly one `if` has no `else`.
//!
//! The fix only applies where the guard does not change which case runs: the block is just the `if`, the case has no
//! guard yet, and the only case after it is a `default` with an empty block. Elsewhere a false guard would go on to
//! the next cases instead of doing nothing. The condition may only name the pattern variables: a guard cannot use
//! local variables that are not effectively final.

use crate::ast::{Case, CaseKind, K, Tree};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};

use super::shared_b::for_each_variable_name;

/// Checks the rules (`case ... ->`) of a `switch`.
pub fn check(switch: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (K::Switch { cases, .. } | K::SwitchExpr { cases, .. }) = &switch.kind else { return };
    for (index, case) in cases.iter().enumerate() {
        if case.kind == CaseKind::Rule {
            check_rule(case, cases.get(index + 1..).unwrap_or_default(), context, diagnostics);
        }
    }
}

fn check_rule(case: &Case, following: &[Case], context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(body) = case.body.as_deref() else { return };
    let K::Block { stmts, .. } = &body.kind else { return };
    let is_pattern = |l: &Tree| matches!(l.kind, K::BindingPattern { .. } | K::RecordPattern { .. });
    let Some(label_end) = case.labels.iter().chain(case.guard.as_deref()).map(|t| t.end).max() else { return };
    let tokens = context.tokens();
    // tree-sitter takes a comment before the body for the body.
    if !case.labels.iter().any(is_pattern) || tokens.has_comment(label_end..body.start) {
        return;
    }
    let statements: Vec<&Tree> = stmts.iter().filter(|s| !matches!(s.kind, K::Empty)).collect();
    if !statements.iter().all(|s| matches!(s.kind, K::If { .. } | K::Break(_))) {
        return;
    }
    let without_else: Vec<&&Tree> = statements.iter().filter(|s| matches!(s.kind, K::If { else_: None, .. })).collect();
    let [guarded] = without_else.as_slice() else { return };
    let message = "'when' expression should be used instead of a single 'if' statement in case block";
    let mut diagnostic = Diagnostic::new(Rule::WhenGuard, case.start, message);
    // Comments and empty statements next to the `if` would be lost.
    let only_the_if = stmts.len() == 1
        && !tokens.has_comment(body.start..guarded.start)
        && !tokens.has_comment(guarded.end..body.end);
    let fixable = case.guard.is_none()
        && only_the_if
        && uses_only_pattern_variables(case, guarded)
        && is_followed_by_empty_default(following, context);
    if fixable {
        diagnostic = diagnostic.with_fix(guard_fix(label_end, body, guarded, context));
    }
    diagnostics.push(diagnostic);
}

/// Whether the condition only refers to the label's pattern variables by name: a guard may only use effectively
/// final local variables, and a name could be a local variable that is not.
fn uses_only_pattern_variables(case: &Case, statement: &Tree) -> bool {
    let mut pattern_variables = Vec::new();
    case.labels.iter().for_each(|l| binding_names(l, &mut pattern_variables));
    let K::If { cond, .. } = &statement.kind else { return false };
    let mut only_patterns = true;
    for_each_variable_name(cond, &mut |name, _| {
        only_patterns &= matches!(&name.kind, K::Ident(n) if pattern_variables.contains(&n.as_str()));
    });
    only_patterns
}

/// The names of the variables a pattern declares, and as in tree-sitter, the simple names of record types.
fn binding_names<'t>(pattern: &'t Tree, names: &mut Vec<&'t str>) {
    match &pattern.kind {
        K::BindingPattern { var } => names.push(&var.name),
        K::RecordPattern { ty, nested } => {
            if let K::Ident(name) = &ty.kind {
                names.push(name);
            }
            nested.iter().for_each(|n| binding_names(n, names));
        }
        _ => {}
    }
}

/// Whether the only case after the rule is `default -> {}`.
fn is_followed_by_empty_default(following: &[Case], context: &Context) -> bool {
    let [default] = following else { return false };
    let Some(body) = default.body.as_deref() else { return false };
    let K::Block { stmts, .. } = &body.kind else { return false };
    default.is_default
        && stmts.iter().all(|s| matches!(s.kind, K::Empty))
        && !context.tokens().has_comment(default.start..body.end)
}

/// `case P -> { if (c) S }` becomes `case P when c -> S'`, where `S'` is `S` as a block.
fn guard_fix(label_end: usize, body: &Tree, statement: &Tree, context: &Context) -> Vec<Edit> {
    let K::If { cond, then, .. } = &statement.kind else { return Vec::new() };
    let inner = context.source(strip_parens(cond));
    let guard = if inner.contains("->") { context.source(cond) } else { inner };
    let replacement = match &then.kind {
        K::Block { .. } => context.source(then).to_string(),
        _ => format!("{{ {} }}", context.source(then)),
    };
    vec![Edit::new(label_end..label_end, format!(" when {guard}")), Edit::new(body.start..body.end, replacement)]
}
