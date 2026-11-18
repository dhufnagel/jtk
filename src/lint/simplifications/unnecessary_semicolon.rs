//! S007: semicolons that do nothing (Checkstyle EmptyStatement and UnnecessarySemicolon*, PMD UnnecessarySemicolon):
//! after a type or member declaration, after the last enum constant, after the last try resource, and empty
//! statements. An empty loop or `if` body is replaced by `{}`.

use std::ops::Range;

use crate::ast::{ClassKind, K, Tree};
use crate::lint::tree::{Context, Tokens};
use crate::lint::{Diagnostic, Edit, Rule};

const AFTER_MEMBER: &str = "Unnecessary semicolon after member declaration";

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let tokens = context.tokens();
    let text = context.text;
    match &tree.kind {
        K::CompilationUnit { package, imports, types, module } => {
            let message = "Unnecessary semicolon after type declaration";
            let package_end = package.as_ref().and_then(|(_, name)| tokens.next(name.end)).map(|s| s.end);
            let items = imports.iter().chain(types).chain(module.as_deref());
            let ends = package_end.into_iter().chain(items.clone().map(|i| i.end));
            let package_start = package.as_ref().and_then(|(annotations, name)| {
                annotations.first().map(|a| a.start).or(tokens.previous(name.start).map(|k| k.start))
            });
            let first = package_start.or(items.clone().next().map(|i| i.start));
            let leading = first.map_or_else(Vec::new, |start| semicolons_before(start, tokens, text));
            for semicolon in leading.into_iter().chain(ends.flat_map(|end| semicolons_after(end, tokens, text))) {
                remove(semicolon, message, diagnostics);
            }
        }
        K::Class(class) => {
            let close = tree.end - 1;
            if class.kind == ClassKind::Enum {
                // The first semicolon ends the constants, and is needed if declarations follow.
                let mut run = semicolons_before(class.members.first().map_or(close, |m| m.start), tokens, text);
                if !run.is_empty() {
                    let first = run.remove(0);
                    if class.members.is_empty() {
                        remove(first, "Unnecessary semicolon after the last enum constant", diagnostics);
                    }
                }
                for semicolon in run.into_iter().chain(after_members(&class.members, tokens, text)) {
                    remove(semicolon, AFTER_MEMBER, diagnostics);
                }
            } else {
                for semicolon in body_semicolons(&class.members, close, tokens, text) {
                    remove(semicolon, AFTER_MEMBER, diagnostics);
                }
            }
            for constant in &class.enum_constants {
                let Some(members) = &constant.body else { continue };
                // The body's `{` follows the name and the arguments in their parentheses.
                let after = constant.args.last().map_or(constant.name_pos + constant.name.len(), |a| a.end);
                let mut following = std::iter::successors(tokens.next(after), |t| tokens.next(t.end));
                let Some(open) = following.find(|t| &text[t.clone()] == "{") else { continue };
                let first_gap = semicolons_after(open.end, tokens, text);
                for semicolon in first_gap.chain(after_members(members, tokens, text)) {
                    remove(semicolon, AFTER_MEMBER, diagnostics);
                }
            }
        }
        K::NewClass { body: Some(members), .. } => {
            for semicolon in body_semicolons(members, tree.end - 1, tokens, text) {
                remove(semicolon, AFTER_MEMBER, diagnostics);
            }
        }
        K::Try { resources, .. } => {
            let trailing = resources.last().and_then(|r| tokens.next(r.end)).filter(|s| &text[s.clone()] == ";");
            if let Some(semicolon) = trailing {
                remove(semicolon, "Unnecessary semicolon after the last resource", diagnostics);
            }
        }
        K::Block { stmts, .. } => report_empty_statements(stmts, diagnostics),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            for case in cases {
                report_empty_statements(&case.stmts, diagnostics);
            }
        }
        K::If { then, else_, .. } => {
            report_empty_body(then, diagnostics);
            if let Some(alternative) = else_ {
                report_empty_body(alternative, diagnostics);
            }
        }
        K::While { body, .. }
        | K::For { body, .. }
        | K::ForEach { body, .. }
        | K::Labeled { body, .. }
        | K::DoWhile { body, .. } => report_empty_body(body, diagnostics),
        _ => {}
    }
}

/// The semicolons between the members of a class body that ends with the `}` at `close`.
fn body_semicolons(members: &[Tree], close: usize, tokens: &Tokens, text: &str) -> Vec<Range<usize>> {
    let mut semicolons = semicolons_before(members.first().map_or(close, |m| m.start), tokens, text);
    semicolons.extend(after_members(members, tokens, text));
    semicolons
}

/// The semicolons right after each member.
fn after_members<'a>(
    members: &'a [Tree],
    tokens: &'a Tokens,
    text: &'a str,
) -> impl Iterator<Item = Range<usize>> + 'a {
    members.iter().flat_map(move |m| semicolons_after(m.end, tokens, text))
}

/// The semicolons that directly follow `offset`.
fn semicolons_after<'a>(offset: usize, tokens: &'a Tokens, text: &'a str) -> impl Iterator<Item = Range<usize>> + 'a {
    std::iter::successors(tokens.next(offset), move |s| tokens.next(s.end)).take_while(move |s| &text[s.clone()] == ";")
}

/// The semicolons that directly precede `offset`, in order.
fn semicolons_before(offset: usize, tokens: &Tokens, text: &str) -> Vec<Range<usize>> {
    let mut semicolons: Vec<Range<usize>> =
        std::iter::successors(tokens.previous(offset), |s| tokens.previous(s.start))
            .take_while(|s| &text[s.clone()] == ";")
            .collect();
    semicolons.reverse();
    semicolons
}

fn report_empty_statements(statements: &[Tree], diagnostics: &mut Vec<Diagnostic>) {
    for statement in statements.iter().filter(|s| matches!(s.kind, K::Empty)) {
        remove(statement.start..statement.end, "Empty statement", diagnostics);
    }
}

/// A `;` that is the body of a statement: replaced by `{}`, as removing it would change the statement.
fn report_empty_body(body: &Tree, diagnostics: &mut Vec<Diagnostic>) {
    if matches!(body.kind, K::Empty) {
        report(body.start..body.end, "Empty statement", "{}", diagnostics);
    }
}

/// Reports a semicolon whose removal is the fix.
fn remove(semicolon: Range<usize>, message: &str, diagnostics: &mut Vec<Diagnostic>) {
    report(semicolon, message, "", diagnostics);
}

fn report(semicolon: Range<usize>, message: &str, replacement: &str, diagnostics: &mut Vec<Diagnostic>) {
    let diagnostic = Diagnostic::new(Rule::UnnecessarySemicolon, semicolon.start, message);
    diagnostics.push(diagnostic.with_fix(vec![Edit::new(semicolon, replacement)]));
}
