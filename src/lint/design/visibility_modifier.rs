//! A003: a field that is not private (Checkstyle VisibilityModifier with its default properties).
//!
//! Allowed are `static final` fields, a public `serialVersionUID`, and fields annotated with JUnit's `@Rule` or
//! `@ClassRule` or Guava's `@VisibleForTesting` (by simple name, unless an import names another type so). Interface
//! and annotation constants are not fields in this sense.

use tree_sitter::Node;

use super::{declarator_names, has_keyword, qualified_name, root};
use crate::lint::syntax::{modifiers, named_children, source};
use crate::lint::{Diagnostic, Rule};

/// The annotations that make a field visible on purpose.
const IGNORED_ANNOTATIONS: &[&str] =
    &["org.junit.Rule", "org.junit.ClassRule", "com.google.common.annotations.VisibleForTesting"];

pub fn check(field: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let allowed = has_keyword(field, text, "private")
        || (has_keyword(field, text, "static") && has_keyword(field, text, "final"))
        || has_ignored_annotation(field, text);
    if allowed {
        return;
    }
    let is_public = has_keyword(field, text, "public");
    for name in declarator_names(field) {
        let name_text = source(&name, text);
        if !(is_public && name_text == "serialVersionUID") {
            let message = format!("Variable '{name_text}' must be private and have accessor methods");
            diagnostics.push(Diagnostic::new(Rule::VisibilityModifier, name.start_byte(), message));
        }
    }
}

fn has_ignored_annotation(field: &Node, text: &str) -> bool {
    let Some(modifiers) = modifiers(field) else { return false };
    named_children(&modifiers).iter().filter(|a| matches!(a.kind(), "annotation" | "marker_annotation")).any(|a| {
        let Some(name) = a.child_by_field_name("name") else { return false };
        let name = qualified_name(&name, text);
        IGNORED_ANNOTATIONS.contains(&name.as_str())
            || IGNORED_ANNOTATIONS.iter().any(|i| i.rsplit('.').next() == Some(name.as_str()))
                && !imports_other_type(field, &name, text)
    })
}

/// Whether the file imports a type with the simple name other than the ignored annotations.
fn imports_other_type(node: &Node, simple_name: &str, text: &str) -> bool {
    named_children(&root(node)).iter().filter(|c| c.kind() == "import_declaration").any(|import| {
        let Some(name) =
            named_children(import).into_iter().find(|c| matches!(c.kind(), "scoped_identifier" | "identifier"))
        else {
            return false;
        };
        let is_star = named_children(import).iter().any(|c| c.kind() == "asterisk");
        let name = qualified_name(&name, text);
        !is_star && name.rsplit('.').next() == Some(simple_name) && !IGNORED_ANNOTATIONS.contains(&name.as_str())
    })
}
