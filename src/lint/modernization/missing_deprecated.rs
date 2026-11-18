//! M013: `@deprecated` Javadoc tag without `@Deprecated` annotation or the other way round, and repeated
//! `@deprecated` tags (Checkstyle MissingDeprecated, Error Prone DepAnn).
//!
//! As in Checkstyle, only the Javadoc comment right before a declaration counts (other comments may come between),
//! and the tag must start a line of it. The fix adds `@Deprecated` where the tag is there; a missing tag is not
//! added (it needs a description). Packages and modules are not fixed. Local classes are not checked.
//!
//! Unlike Checkstyle, Javadoc that Checkstyle's Javadoc parser rejects (unclosed HTML tags, ...) is still checked;
//! Checkstyle reports a parse error under this check's name instead.

use std::ops::Range;

use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};

/// Checks the declarations in a compilation unit, class body or anonymous class body.
pub fn check(container: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    match &container.kind {
        K::CompilationUnit { package, types, module, .. } => {
            if let Some((annotations, name)) = package {
                let start = annotations
                    .first()
                    .map(|a| a.start)
                    .or_else(|| context.tokens().previous(name.start).map(|keyword| keyword.start));
                if let Some(start) = start {
                    check_declaration(start, annotations, false, context, diagnostics);
                }
            }
            types.iter().for_each(|t| check_member(t, context, diagnostics));
            if let Some(module) = module
                && let K::Module { annotations, .. } = &module.kind
            {
                check_declaration(module.start, annotations, false, context, diagnostics);
            }
        }
        K::Class(class) => {
            for constant in &class.enum_constants {
                check_declaration(constant.mods.start, &constant.mods.annotations, true, context, diagnostics);
                constant.body.iter().flatten().for_each(|m| check_member(m, context, diagnostics));
            }
            class.members.iter().for_each(|m| check_member(m, context, diagnostics));
        }
        K::NewClass { body: Some(body), .. } => body.iter().for_each(|m| check_member(m, context, diagnostics)),
        _ => {}
    }
}

fn check_member(member: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let annotations = match &member.kind {
        K::Class(class) => &class.mods.annotations,
        K::Method(method) => &method.mods.annotations,
        K::Vars(vars) if !vars.is_empty() => &vars[0].mods.annotations,
        _ => return,
    };
    check_declaration(member.start, annotations, true, context, diagnostics);
}

fn check_declaration(
    start: usize,
    annotations: &[Tree],
    fixable: bool,
    context: &Context,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(javadoc) = javadoc_before(start, context) else { return };
    let text = context.text;
    let tags = deprecated_tags(javadoc, text);
    for &duplicate in tags.iter().skip(1) {
        diagnostics.push(Diagnostic::new(Rule::MissingDeprecated, duplicate, "Duplicate @deprecated tag"));
    }
    let annotated = annotations.iter().any(|a| {
        let K::Annotation { ty, .. } = &a.kind else { return false };
        let name: String = context.source(ty).split_whitespace().collect();
        matches!(name.as_str(), "Deprecated" | "java.lang.Deprecated")
    });
    if annotated == !tags.is_empty() {
        return;
    }
    let message = "Must include both @java.lang.Deprecated annotation and @deprecated Javadoc tag with description";
    let mut diagnostic = Diagnostic::new(Rule::MissingDeprecated, start, message);
    if !annotated && fixable {
        let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
        let indentation = &text[line_start..start];
        let annotation = if indentation.trim().is_empty() {
            format!("@Deprecated\n{indentation}")
        } else {
            "@Deprecated ".to_string()
        };
        diagnostic = diagnostic.with_fix(vec![Edit::new(start..start, annotation)]);
    }
    diagnostics.push(diagnostic);
}

/// The last Javadoc comment among the comments directly before `start`.
fn javadoc_before(start: usize, context: &Context) -> Option<Range<usize>> {
    let tokens = context.tokens();
    let after = tokens.previous(start).map_or(0, |token| token.end);
    tokens.comments_in(after..start).filter(|c| is_javadoc(&context.text[c.clone()])).last()
}

fn is_javadoc(comment: &str) -> bool {
    comment.starts_with("/**") && comment != "/**/"
}

/// The offsets of the `@deprecated` block tags: those starting a line of the comment (after any `*`).
fn deprecated_tags(javadoc: Range<usize>, text: &str) -> Vec<usize> {
    let comment = &text[javadoc.clone()];
    let content = &comment[3..comment.len().saturating_sub(2).max(3)];
    let mut tags = Vec::new();
    let mut offset = javadoc.start + 3;
    for line in content.split_inclusive('\n') {
        let rest = line.trim_start_matches([' ', '\t']).trim_start_matches('*').trim_start_matches([' ', '\t']);
        if let Some(after) = rest.strip_prefix("@deprecated")
            && after.chars().next().is_none_or(char::is_whitespace)
        {
            tags.push(offset + line.len() - rest.len());
        }
        offset += line.len();
    }
    tags
}
