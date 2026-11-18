//! A008: overloaded methods or constructors that are not declared next to each other (Checkstyle
//! OverloadMethodsDeclarationOrder and ConstructorsDeclarationGrouping).
//!
//! As in Checkstyle, a method is reported if the previous method with its name is not directly before it, and
//! constructors (of classes, enums and records) after the first member that follows the first constructor. Stray
//! semicolons between members count as members. The methods of annotation types and of enum constant bodies are not
//! checked.

use std::collections::HashMap;
use std::ops::Range;

use crate::ast::{ClassKind, K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (members, has_constructors) = match &tree.kind {
        K::Class(class) if class.kind != ClassKind::Annotation => (&class.members, class.kind != ClassKind::Interface),
        K::NewClass { body: Some(body), .. } => (body, false),
        _ => return,
    };
    check_methods(members, context, diagnostics);
    if has_constructors {
        check_constructors(members, context, diagnostics);
    }
}

fn check_methods(members: &[Tree], context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    // The index in `members` and the start of the last method with each name.
    let mut last: HashMap<&str, (usize, usize)> = HashMap::new();
    for (i, member) in members.iter().enumerate() {
        let K::Method(method) = &member.kind else { continue };
        if method.ret.is_none() {
            continue;
        }
        if let Some(&(previous, previous_start)) = last.get(method.name.as_str())
            && (previous + 1 != i || has_semicolon(members[previous].end..member.start, context))
        {
            let message = format!(
                "All overloaded methods should be placed next to each other. Previous overloaded method located at \
                 line {}",
                line(previous_start, context.text)
            );
            diagnostics.push(Diagnostic::new(Rule::OverloadOrder, member.start, message));
        }
        last.insert(&method.name, (i, member.start));
    }
}

fn check_constructors(members: &[Tree], context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let is_constructor = |m: &Tree| matches!(&m.kind, K::Method(method) if method.ret.is_none());
    let Some(first) = members.iter().position(is_constructor) else { return };
    let mut group_end = first + 1;
    while group_end < members.len()
        && is_constructor(&members[group_end])
        && !has_semicolon(members[group_end - 1].end..members[group_end].start, context)
    {
        group_end += 1;
    }
    let mut later = members[group_end..].iter().filter(|m| is_constructor(m)).peekable();
    if later.peek().is_none() {
        return;
    }
    let last_grouped = line(members[group_end - 1].start, context.text);
    for constructor in later {
        let message = format!(
            "Constructors should be grouped together. The last grouped constructor is declared at line {last_grouped}"
        );
        diagnostics.push(Diagnostic::new(Rule::OverloadOrder, constructor.start, message));
    }
}

/// Whether a stray semicolon, which counts as a member, is between two members.
fn has_semicolon(between: Range<usize>, context: &Context) -> bool {
    context.tokens().in_range(between).any(|token| &context.text[token] == ";")
}

/// The line (from 1) of an offset.
fn line(offset: usize, text: &str) -> usize {
    text.as_bytes()[..offset].iter().filter(|&&b| b == b'\n').count() + 1
}
