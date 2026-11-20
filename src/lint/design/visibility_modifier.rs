//! A003: a field that is not private (Checkstyle VisibilityModifier with its default properties).
//!
//! Allowed are `static final` fields, a public `serialVersionUID`, and fields annotated with JUnit's `@Rule` or
//! `@ClassRule` or Guava's `@VisibleForTesting` (by simple name, unless an import names another type so). Interface
//! and annotation constants are not fields in this sense.

use super::{has_keyword, qualified_name, simple_name, single_type_imports};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Tree};

/// The annotations that make a field visible on purpose.
const IGNORED_ANNOTATIONS: &[&str] =
    &["org.junit.Rule", "org.junit.ClassRule", "com.google.common.annotations.VisibleForTesting"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Vars(vars) = &tree.kind else { return };
    let is_field = match context.parent().map(|p| &p.kind) {
        Some(K::Class(class)) => !matches!(class.kind, ClassKind::Interface | ClassKind::Annotation),
        Some(K::NewClass { .. }) => true,
        _ => false,
    };
    if !is_field {
        return;
    }
    let mods = &vars[0].mods;
    let has = |keyword| has_keyword(mods, vars[0].name_pos, keyword, context);
    if has("private") || (has("static") && has("final")) || has_ignored_annotation(&mods.annotations, context) {
        return;
    }
    let is_public = has("public");
    for var in vars {
        if !(is_public && var.name == "serialVersionUID") {
            let message = format!("Variable '{}' must be private and have accessor methods", var.name);
            diagnostics.push(Diagnostic::new(Rule::VisibilityModifier, var.name_pos, message));
        }
    }
}

fn has_ignored_annotation(annotations: &[Tree], context: &Context) -> bool {
    annotations.iter().any(|annotation| {
        let K::Annotation { ty, .. } = &annotation.kind else { return false };
        let name = qualified_name(ty);
        IGNORED_ANNOTATIONS.contains(&name.as_str())
            || IGNORED_ANNOTATIONS.iter().any(|i| simple_name(i) == name) && !imports_other_type(&name, context)
    })
}

/// Whether the file imports a type with the simple name other than the ignored annotations.
fn imports_other_type(simple: &str, context: &Context) -> bool {
    single_type_imports(context.ancestors[0])
        .any(|name| simple_name(&name) == simple && !IGNORED_ANNOTATIONS.contains(&name.as_str()))
}
