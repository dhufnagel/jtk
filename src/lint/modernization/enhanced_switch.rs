//! M002: `switch` with `case ...:` groups that could use the arrow form (`case ... ->`) because no group falls
//! through (Checkstyle UseEnhancedSwitch, Eclipse save action "Convert to switch expression" in part), Java 14.
//!
//! As in Checkstyle, every group but the last must end in a statement that cannot complete normally (`return`,
//! `throw`, `yield`, `break`, `continue`, or an `if`, `try`, loop, `switch` or block ending so). Only the group's
//! last statement counts.
//!
//! Unsafe fix, for `switch` statements whose groups each end in `break` (removed), `return`, `throw` or `continue`
//! (the last group also without one), with no other `break` of the switch and no local variable used in another
//! group: each group becomes a rule (`case 1, 2 -> { ... }`). Not with comments outside the statements, `default`
//! together with other labels, or several pattern labels. Unsafe because the checks are syntactic: whether a
//! variable declared in one group is used in another is judged by its name.

use std::ops::Range;

use crate::ast::{Case, CaseKind, K, LitKind, Tree};
use crate::lint::tree::{Context, Tokens, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(switch: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (K::Switch { selector, cases } | K::SwitchExpr { selector, cases }) = &switch.kind else { return };
    if cases.iter().any(|c| c.kind == CaseKind::Rule) {
        return;
    }
    let groups = groups(cases);
    let Some(last_group) = groups.last() else { return };
    // Labels without statements form one group with the next ones, as in Checkstyle.
    let last_statements: Vec<&Tree> = groups.iter().filter_map(|g| last_case(g).stmts.last()).collect();
    let leading = match last_case(last_group).stmts.last() {
        Some(_) => &last_statements[..last_statements.len() - 1],
        None => &last_statements[..],
    };
    let ends_group = |s: &Tree| terminates(s, Exits { by_break: true, by_continue: true }, &mut Vec::new(), 0);
    if leading.iter().all(|s| ends_group(s)) {
        let message = "Switch can be replaced with enhanced switch";
        let diagnostic = Diagnostic::new(Rule::EnhancedSwitch, switch.start, message);
        let is_statement = matches!(switch.kind, K::Switch { .. })
            && context.parent().is_some_and(|p| {
                matches!(
                    p.kind,
                    K::Block { .. }
                        | K::Switch { .. }
                        | K::SwitchExpr { .. }
                        | K::Labeled { .. }
                        | K::If { .. }
                        | K::While { .. }
                        | K::For { .. }
                        | K::ForEach { .. }
                        | K::DoWhile { .. }
                )
            });
        let fix = is_statement.then(|| arrow_form(switch, selector, &groups, context)).flatten();
        diagnostics.push(match fix {
            Some(edit) => diagnostic.with_unsafe_fix(vec![edit]),
            None => diagnostic,
        });
    }
}

/// The statement groups of a `switch` with colon cases: the cases up to one with statements, and the cases without
/// statements at the end.
fn groups(cases: &[Case]) -> Vec<&[Case]> {
    let mut groups = Vec::new();
    let mut start = 0;
    for (index, case) in cases.iter().enumerate() {
        if !case.stmts.is_empty() || index == cases.len() - 1 {
            groups.push(&cases[start..=index]);
            start = index + 1;
        }
    }
    groups
}

fn last_case(group: &[Case]) -> &Case {
    &group[group.len() - 1]
}

/// The statements of a group or block without empty statements.
fn statements(statements: &[Tree]) -> Vec<&Tree> {
    statements.iter().filter(|s| !matches!(s.kind, K::Empty)).collect()
}

/// How a statement can leave the `case` group.
#[derive(Clone, Copy)]
struct Exits {
    by_break: bool,
    by_continue: bool,
}

/// Whether control cannot reach the end of a statement of a `case` group, so that the group does not fall through:
/// a `break` or `continue` leaves the group unless it targets a statement within it. `labels` are the labels of
/// the statements within the group that enclose the statement, the last `direct` of them directly.
fn terminates<'t>(statement: &'t Tree, exits: Exits, labels: &mut Vec<&'t str>, direct: usize) -> bool {
    let jumps_out = |label: &Option<String>, unlabeled_leaves: bool, labels: &[&str]| match label {
        None => unlabeled_leaves,
        Some(label) => !labels.contains(&label.as_str()),
    };
    match &statement.kind {
        K::Return(_) | K::Throw(_) | K::Yield(_) => true,
        K::Break(label) => jumps_out(label, exits.by_break, labels),
        K::Continue(label) => jumps_out(label, exits.by_continue, labels),
        K::Block { stmts, .. } => statements(stmts).last().is_some_and(|s| terminates(s, exits, labels, 0)),
        K::If { then, else_: Some(alternative), .. } => {
            terminates(then, exits, labels, 0) && terminates(alternative, exits, labels, 0)
        }
        K::Try { block, catches, finally, .. } => {
            finally.as_deref().is_some_and(|f| terminates(f, exits, labels, 0))
                || terminates(block, exits, labels, 0) && catches.iter().all(|c| terminates(&c.block, exits, labels, 0))
        }
        K::Synchronized { block, .. } => terminates(block, exits, labels, 0),
        K::Labeled { label, body } => {
            labels.push(label);
            let terminates = terminates(body, exits, labels, direct + 1);
            labels.pop();
            terminates
        }
        K::While { .. } | K::DoWhile { .. } | K::For { .. } | K::ForEach { .. } => {
            loop_terminates(statement, labels, direct)
        }
        K::Switch { cases, .. } => switch_terminates(cases, exits, labels),
        K::ExprStmt(expression) => match &expression.kind {
            K::SwitchExpr { cases, .. } => switch_terminates(cases, exits, labels),
            _ => false,
        },
        _ => false,
    }
}

/// As in Checkstyle, a loop ends the case if its body does, or if it is an infinite loop without `break`.
fn loop_terminates<'t>(statement: &'t Tree, labels: &mut Vec<&'t str>, direct: usize) -> bool {
    let (is_infinite, body) = match &statement.kind {
        K::For { cond, body, .. } => (cond.as_deref().is_none_or(is_true), body),
        K::ForEach { body, .. } => (false, body),
        K::While { cond, body } | K::DoWhile { cond, body } => (is_true(cond), body),
        _ => return false,
    };
    let has_break = has_break(body, &labels[labels.len() - direct..]);
    let body_exits = Exits { by_break: false, by_continue: false };
    (is_infinite && !has_break) || terminates(body, body_exits, labels, 0)
}

fn is_true(condition: &Tree) -> bool {
    let condition = strip_parens(condition);
    matches!(condition.kind, K::Literal(LitKind::Boolean)) && condition.end - condition.start == "true".len()
}

/// Whether a `break` leaves the loop: an unlabeled one outside nested loops and switches, or one naming one of the
/// loop's labels. As in Checkstyle, a loop or `switch` that is the body itself does not count as nested.
fn has_break(body: &Tree, loop_labels: &[&str]) -> bool {
    fn breaks(tree: &Tree, nested: bool, loop_labels: &[&str]) -> bool {
        match &tree.kind {
            K::Break(None) => !nested,
            K::Break(Some(label)) => loop_labels.contains(&label.as_str()),
            K::Lambda { .. } | K::Class(_) | K::NewClass { .. } => false,
            _ => {
                let nested = nested || is_break_target(tree);
                let mut found = false;
                tree.for_each_child(&mut |child| found = found || breaks(child, nested, loop_labels));
                found
            }
        }
    }
    let nested = matches!(body.kind, K::Switch { .. } | K::SwitchExpr { .. });
    match &body.kind {
        K::Break(_) => breaks(body, false, loop_labels),
        _ => {
            let mut found = false;
            body.for_each_child(&mut |child| found = found || breaks(child, nested, loop_labels));
            found
        }
    }
}

fn is_break_target(tree: &Tree) -> bool {
    matches!(
        tree.kind,
        K::For { .. }
            | K::ForEach { .. }
            | K::While { .. }
            | K::DoWhile { .. }
            | K::Switch { .. }
            | K::SwitchExpr { .. }
    )
}

/// A nested `switch` ends the case if it has a `default` and all its groups end it (a `break` only leaves the nested
/// switch).
fn switch_terminates<'t>(cases: &'t [Case], exits: Exits, labels: &mut Vec<&'t str>) -> bool {
    let nested = Exits { by_break: false, ..exits };
    cases.iter().any(|c| c.is_default)
        && cases.iter().all(|c| c.kind == CaseKind::Statement)
        && groups(cases)
            .iter()
            .all(|g| statements(&last_case(g).stmts).last().is_some_and(|s| terminates(s, nested, labels, 0)))
}

/// The edit that writes the body of a `switch` statement with rules, if that is simple and keeps the meaning.
fn arrow_form(switch: &Tree, selector: &Tree, groups: &[&[Case]], context: &Context) -> Option<Edit> {
    let (text, tokens) = (context.text, context.tokens());
    let open = tokens.next(selector.end)?;
    let spans: Vec<Range<usize>> = groups.iter().map(|g| group_span(g, tokens)).collect();
    // Comments between the groups would be lost.
    if tokens.comments_in(open.end..switch.end).any(|c| !spans.iter().any(|s| s.contains(&c.start))) {
        return None;
    }
    let mut rules = Vec::new();
    let mut labels: Vec<&Case> = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        labels.extend(group.iter());
        let mut statements = statements(&last_case(group).stmts);
        // The comments that are not in a label or statement.
        let mut loose = loose_comments(spans[index].clone(), group, tokens);
        // `case 1: { ...; break; }`: the block's statements.
        if let [block] = statements.as_slice()
            && let K::Block { stmts, .. } = &block.kind
            && stmts.last().is_some_and(|s| matches!(s.kind, K::Break(_)))
        {
            loose = tokens
                .comments_in(block.start..block.end)
                .filter(|c| !stmts.iter().any(|s| s.start <= c.start && c.start < s.end))
                .collect();
            statements = self::statements(stmts);
        }
        let is_last_group = index == groups.len() - 1;
        let Some(last) = statements.last() else {
            if is_last_group {
                // Labels at the end without statements.
                rules.push(format!("{} -> {{}}", rule_labels(&labels, context)?));
            }
            continue;
        };
        let kept = match last.kind {
            K::Break(None) => &statements[..statements.len() - 1],
            K::Return(_) | K::Throw(_) | K::Continue(_) => &statements[..],
            _ if is_last_group => &statements[..],
            _ => return None,
        };
        // Comments must be among the kept statements, which are copied with what is between them.
        // Comments before a removed `break` are kept too.
        let end = match kept.last() {
            Some(_) if kept.len() < statements.len() => text[..last.start].trim_end().len(),
            Some(l) => l.end,
            None => 0,
        };
        let kept_range = kept.first().map_or(0..0, |f| f.start..end);
        let comments_outside = loose.iter().any(|c| !kept_range.contains(&c.start));
        if comments_outside || kept.iter().any(|s| breaks_switch(s)) || declares_shared(kept, index, &spans, text) {
            return None;
        }
        let code = match kept {
            [] => "{}".to_string(),
            [single]
                if matches!(single.kind, K::ExprStmt(_) | K::Throw(_) | K::Block { .. })
                    && kept_range.end == single.end =>
            {
                context.source(single).to_string()
            }
            _ => format!("{{\n{}\n}}", &text[kept_range]),
        };
        rules.push(format!("{} -> {code}", rule_labels(&labels, context)?));
        labels.clear();
    }
    Some(Edit::new(open.start..switch.end, format!("{{\n{}\n}}", rules.join("\n"))))
}

/// The end of a case's label (`case 1, 2` or `default`), before the colon.
fn label_end(case: &Case, tokens: &Tokens) -> usize {
    let items = case.labels.iter().chain(case.guard.as_deref());
    items.map(|t| t.end).max().or_else(|| tokens.next(case.start).map(|keyword| keyword.end)).unwrap_or(case.start)
}

/// The range of a group, from its first label to its last statement or colon.
fn group_span(group: &[Case], tokens: &Tokens) -> Range<usize> {
    let last = last_case(group);
    let end = match last.stmts.last() {
        Some(statement) => statement.end,
        None => tokens.next(label_end(last, tokens)).map_or(last.start, |colon| colon.end),
    };
    group[0].start..end
}

/// The comments in a group's span that are neither in a label nor in a statement.
fn loose_comments(span: Range<usize>, group: &[Case], tokens: &Tokens) -> Vec<Range<usize>> {
    let in_label = |c: &Range<usize>| group.iter().any(|g| g.start <= c.start && c.start < label_end(g, tokens));
    let in_statement =
        |c: &Range<usize>| group.iter().flat_map(|g| &g.stmts).any(|s| s.start <= c.start && c.start < s.end);
    tokens.comments_in(span).filter(|c| !in_label(c) && !in_statement(c)).collect()
}

/// The labels of a rule: `case A, B` or `default`.
fn rule_labels(labels: &[&Case], context: &Context) -> Option<String> {
    let mut cases = Vec::new();
    let mut patterns = 0;
    for label in labels {
        if label.is_default {
            // `default` with other labels.
            return (labels.len() == 1).then(|| "default".to_string());
        }
        let code = &context.text[label.start..label_end(label, context.tokens())];
        cases.push(code.strip_prefix("case").unwrap_or(code).trim().to_string());
        let is_pattern = |l: &Tree| matches!(l.kind, K::BindingPattern { .. } | K::RecordPattern { .. });
        patterns += usize::from(label.labels.iter().any(is_pattern) || label.guard.is_some());
    }
    (patterns <= usize::from(cases.len() == 1)).then(|| format!("case {}", cases.join(", ")))
}

/// Whether a `break` in the statement ends the switch (not a loop or switch inside it).
fn breaks_switch(statement: &Tree) -> bool {
    match &statement.kind {
        K::Break(label) => label.is_none(),
        K::Lambda { .. } | K::Class(_) | K::NewClass { .. } => false,
        _ if is_break_target(statement) => false,
        _ => {
            let mut found = false;
            statement.for_each_child(&mut |child| found = found || breaks_switch(child));
            found
        }
    }
}

/// Whether a variable declared by the statements of a group is named in another group: in a rule it would be out of
/// scope there.
fn declares_shared(statements: &[&Tree], group: usize, spans: &[Range<usize>], text: &str) -> bool {
    let names = statements.iter().flat_map(|s| match &s.kind {
        K::Vars(vars) => vars.as_slice(),
        _ => &[],
    });
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    names.map(|v| v.name.as_str()).any(|name| {
        spans.iter().enumerate().filter(|(i, _)| *i != group).any(|(_, span)| {
            let code = &text[span.clone()];
            code.match_indices(name).any(|(i, _)| {
                !code[..i].chars().next_back().is_some_and(is_word)
                    && !code[i + name.len()..].chars().next().is_some_and(is_word)
            })
        })
    })
}
