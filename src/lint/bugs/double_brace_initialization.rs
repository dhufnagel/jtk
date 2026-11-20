//! B025: an anonymous class whose body holds only instance initializers, `new ArrayList<>() {{ add(1); }}`
//! (Checkstyle AvoidDoubleBraceInitialization, PMD DoubleBraceInitialization): it creates a class per use and keeps
//! a reference to the enclosing instance.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::NewClass { ident, args, body: Some(members), .. } = &tree.kind else { return };
    let Some(first) = members.first() else { return };
    if !members.iter().all(|m| matches!(m.kind, K::Block { is_static: false, .. })) {
        return;
    }
    // The body's brace is the first after the arguments.
    let arguments_end = args.last().map_or(ident.end, |a| a.end);
    let text = context.text;
    let brace = context.tokens().in_range(arguments_end..first.start).find(|t| &text[t.clone()] == "{");
    diagnostics.push(Diagnostic::new(
        Rule::DoubleBraceInitialization,
        brace.map_or(first.start, |b| b.start),
        "Avoid double brace initialization",
    ));
}
