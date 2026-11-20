//! S013: several variables in one declaration (Checkstyle MultipleVariableDeclarations, PMD OneDeclarationPerLine).
//! Declarations in a `for` initializer are allowed. Several declarations on one line are left to the formatter,
//! which puts each statement on its own line.
//!
//! The fix splits a declaration into one per variable, each with the modifiers, annotations and type; array
//! dimensions after a name move to its type. Declarations with comments inside are not split.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree, VarDecl};

pub fn check(container: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    match &container.kind {
        K::Block { stmts, .. } => check_list(stmts, context, diagnostics),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            for case in cases {
                check_list(&case.stmts, context, diagnostics);
            }
        }
        K::Class(class) => {
            check_list(&class.members, context, diagnostics);
            for body in class.enum_constants.iter().filter_map(|c| c.body.as_deref()) {
                check_list(body, context, diagnostics);
            }
        }
        K::NewClass { body: Some(members), .. } => check_list(members, context, diagnostics),
        _ => {}
    }
}

/// Checks the declarations among the statements or members.
fn check_list(children: &[Tree], context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    for declaration in children {
        let K::Vars(declarators) = &declaration.kind else { continue };
        if declarators.len() > 1 {
            let message = "Each variable declaration must be in its own statement";
            let mut diagnostic = Diagnostic::new(Rule::MultipleDeclarations, declaration.start, message);
            if let Some(split) = split(declaration, declarators, context) {
                diagnostic = diagnostic.with_fix(vec![Edit::new(declaration.start..declaration.end, split)]);
            }
            diagnostics.push(diagnostic);
        }
    }
}

/// The declaration split into one per variable, on separate lines with the indentation of the first one.
fn split(declaration: &Tree, declarators: &[VarDecl], context: &Context) -> Option<String> {
    let (text, tokens) = (context.text, context.tokens());
    if tokens.has_comment(declaration.start..declaration.end) || declarators[0].ty.is_none() {
        return None;
    }
    // The type ends before the first name; C-style dimensions (`a[]`) extend a variable's type past its name.
    let type_end = tokens.previous(declarators[0].name_pos)?.end;
    let prefix = &text[declaration.start..type_end];
    let line_start = text[..declaration.start].rfind('\n').map_or(0, |i| i + 1);
    let before = &text[line_start..declaration.start];
    let separator = if before.trim().is_empty() { format!("\n{before}") } else { " ".to_string() };
    let parts: Vec<String> = declarators
        .iter()
        .map(|declarator| {
            let name_end = declarator.name_pos + declarator.name.len();
            let dimensions_end = declarator.ty.as_deref().map_or(name_end, |t| t.end.max(name_end));
            let dimensions = &text[name_end..dimensions_end];
            if dimensions.contains('@') {
                // Annotated dimensions (`a @A []`).
                return None;
            }
            let dimensions: String = dimensions.split_whitespace().collect();
            Some(format!("{prefix}{dimensions} {}{};", declarator.name, &text[dimensions_end..declarator.end]))
        })
        .collect::<Option<_>>()?;
    Some(parts.join(&separator))
}
