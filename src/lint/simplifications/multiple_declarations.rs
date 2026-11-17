//! S013: several variables in one declaration, or several declarations on one line (Checkstyle
//! MultipleVariableDeclarations, PMD OneDeclarationPerLine). Declarations in a `for` initializer are allowed.
//!
//! The fix splits a declaration into one per variable, each with the modifiers, annotations and type; array
//! dimensions after a name move to its type. Declarations with comments inside are not split. Separate declarations
//! on one line are left to the formatter.

use tree_sitter::Node;

use super::contains_comment;
use crate::lint::syntax::{is_comment, named_children, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(container: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let children: Vec<Node> = named_children(container).into_iter().filter(|c| !is_comment(c)).collect();
    for (index, declaration) in children.iter().enumerate() {
        if !is_declaration(declaration) {
            continue;
        }
        let declarators = declarators(declaration);
        if declarators.len() > 1 {
            let message = "Each variable declaration must be in its own statement";
            let mut diagnostic = Diagnostic::new(Rule::MultipleDeclarations, declaration.start_byte(), message);
            if let Some(split) = split(declaration, &declarators, text) {
                diagnostic = diagnostic.with_fix(vec![Edit::new(declaration.byte_range(), split)]);
            }
            diagnostics.push(diagnostic);
        }
        let next_on_same_line = children
            .get(index + 1)
            .is_some_and(|next| is_declaration(next) && next.start_position().row == declaration.end_position().row);
        if next_on_same_line {
            let message = "Only one variable definition per line allowed";
            diagnostics.push(Diagnostic::new(Rule::MultipleDeclarations, declaration.start_byte(), message));
        }
    }
}

fn is_declaration(node: &Node) -> bool {
    matches!(node.kind(), "field_declaration" | "constant_declaration" | "local_variable_declaration")
}

fn declarators<'t>(declaration: &Node<'t>) -> Vec<Node<'t>> {
    named_children(declaration).into_iter().filter(|d| d.kind() == "variable_declarator").collect()
}

/// The declaration split into one per variable, on separate lines with the indentation of the first one.
fn split(declaration: &Node, declarators: &[Node], text: &str) -> Option<String> {
    if contains_comment(declaration) {
        return None;
    }
    let declared_type = declaration.child_by_field_name("type")?;
    if source(&declared_type, text) == "var" {
        return None;
    }
    let prefix = &text[declaration.start_byte()..declared_type.end_byte()];
    let line_start = text[..declaration.start_byte()].rfind('\n').map_or(0, |i| i + 1);
    let before = &text[line_start..declaration.start_byte()];
    let separator = if before.trim().is_empty() { format!("\n{before}") } else { " ".to_string() };
    let parts: Vec<String> = declarators
        .iter()
        .map(|declarator| {
            let name = declarator.child_by_field_name("name")?;
            let (dimensions, rest) = match declarator.child_by_field_name("dimensions") {
                Some(dimensions) => {
                    if !named_children(&dimensions).is_empty() {
                        // Annotated dimensions (`a @A []`).
                        return None;
                    }
                    let dimensions_code: String = source(&dimensions, text).split_whitespace().collect();
                    (dimensions_code, &text[dimensions.end_byte()..declarator.end_byte()])
                }
                None => (String::new(), &text[name.end_byte()..declarator.end_byte()]),
            };
            Some(format!("{prefix}{dimensions} {}{rest};", source(&name, text)))
        })
        .collect::<Option<_>>()?;
    Some(parts.join(&separator))
}
