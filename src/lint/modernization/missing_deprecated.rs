//! M013: `@deprecated` Javadoc tag without `@Deprecated` annotation or the other way round, and repeated
//! `@deprecated` tags (Checkstyle MissingDeprecated, Error Prone DepAnn).
//!
//! As in Checkstyle, only the Javadoc comment right before a declaration counts (other comments may come between),
//! and the tag must start a line of it. The fix adds `@Deprecated` where the tag is there; a missing tag is not
//! added (it needs a description). Packages and modules are not fixed.
//!
//! Unlike Checkstyle, Javadoc that Checkstyle's Javadoc parser rejects (unclosed HTML tags, ...) is still checked;
//! Checkstyle reports a parse error under this check's name instead.

use tree_sitter::Node;

use crate::lint::syntax::{is_comment, modifiers, named_children, source};
use crate::lint::{Diagnostic, Edit, Rule};

const DECLARATIONS: &[&str] = &[
    "class_declaration",
    "interface_declaration",
    "enum_declaration",
    "annotation_type_declaration",
    "record_declaration",
    "method_declaration",
    "constructor_declaration",
    "compact_constructor_declaration",
    "field_declaration",
    "constant_declaration",
    "enum_constant",
    "annotation_type_element_declaration",
    "package_declaration",
    "module_declaration",
];

/// Checks the declarations in a compilation unit or type body.
pub fn check(container: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut javadoc: Option<Node> = None;
    let mut cursor = container.walk();
    for child in container.children(&mut cursor) {
        if is_comment(&child) {
            if source(&child, text).starts_with("/**") && source(&child, text) != "/**/" {
                javadoc = Some(child);
            }
            continue;
        }
        if let Some(comment) = javadoc.take().filter(|_| DECLARATIONS.contains(&child.kind())) {
            check_declaration(&child, &comment, text, diagnostics);
        }
    }
}

fn check_declaration(declaration: &Node, javadoc: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let tags = deprecated_tags(javadoc, text);
    for &duplicate in tags.iter().skip(1) {
        diagnostics.push(Diagnostic::new(Rule::MissingDeprecated, duplicate, "Duplicate @deprecated tag"));
    }
    let annotated = annotations(declaration).iter().any(|a| {
        let name = a.child_by_field_name("name").map(|n| source(&n, text).split_whitespace().collect::<String>());
        matches!(name.as_deref(), Some("Deprecated" | "java.lang.Deprecated"))
    });
    if annotated == !tags.is_empty() {
        return;
    }
    let message = "Must include both @java.lang.Deprecated annotation and @deprecated Javadoc tag with description";
    let mut diagnostic = Diagnostic::new(Rule::MissingDeprecated, declaration.start_byte(), message);
    if !annotated && !matches!(declaration.kind(), "package_declaration" | "module_declaration") {
        let start = declaration.start_byte();
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

fn annotations<'t>(declaration: &Node<'t>) -> Vec<Node<'t>> {
    let holder = if declaration.kind() == "package_declaration" { Some(*declaration) } else { modifiers(declaration) };
    holder
        .map(|h| named_children(&h).into_iter().filter(|c| matches!(c.kind(), "annotation" | "marker_annotation")))
        .into_iter()
        .flatten()
        .collect()
}

/// The offsets of the `@deprecated` block tags: those starting a line of the comment (after any `*`).
fn deprecated_tags(javadoc: &Node, text: &str) -> Vec<usize> {
    let comment = source(javadoc, text);
    let content = &comment[3..comment.len().saturating_sub(2).max(3)];
    let mut tags = Vec::new();
    let mut offset = javadoc.start_byte() + 3;
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
