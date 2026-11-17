//! S014: C-style array brackets after a variable name or method parameter list, `int a[]` instead of `int[] a`
//! (Checkstyle ArrayTypeStyle with the default Java style). Like Checkstyle, brackets on a later line than the name
//! are only reported if they start right of the name's column.
//!
//! The fix moves the brackets to the type, unless the declaration declares several variables (whose types would
//! change too) or the brackets are annotated or contain comments.

use tree_sitter::Node;

use crate::lint::syntax::named_children;
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(declaration: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    match declaration.kind() {
        "field_declaration" | "constant_declaration" | "local_variable_declaration" => {
            let declarators: Vec<Node> =
                named_children(declaration).into_iter().filter(|d| d.kind() == "variable_declarator").collect();
            for declarator in &declarators {
                if let (Some(name), Some(dimensions)) =
                    (declarator.child_by_field_name("name"), declarator.child_by_field_name("dimensions"))
                {
                    report(declaration, &name, &dimensions, declarators.len() == 1, text, diagnostics);
                }
            }
        }
        _ => {
            if let (Some(name), Some(dimensions)) =
                (declaration.child_by_field_name("name"), declaration.child_by_field_name("dimensions"))
            {
                report(declaration, &name, &dimensions, true, text, diagnostics);
            }
        }
    }
}

fn report(
    declaration: &Node,
    name: &Node,
    dimensions: &Node,
    fixable: bool,
    text: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let fix = declaration.child_by_field_name("type").filter(|_| fixable && named_children(dimensions).is_empty()).map(
        |declared_type| {
            let brackets = "[]".repeat(dimensions.children(&mut dimensions.walk()).filter(|c| c.kind() == "[").count());
            let whitespace = text[..dimensions.start_byte()].len() - text[..dimensions.start_byte()].trim_end().len();
            vec![
                Edit::new(declared_type.end_byte()..declared_type.end_byte(), brackets),
                Edit::new(dimensions.start_byte() - whitespace..dimensions.end_byte(), ""),
            ]
        },
    );
    // Like Checkstyle, brackets on a later line than the name count as following the type if they start left of it.
    let is_after_name = |bracket: &Node| {
        bracket.start_position().row == name.start_position().row || column(name, text) < column(bracket, text)
    };
    for bracket in dimensions.children(&mut dimensions.walk()).filter(|c| c.kind() == "[" && is_after_name(c)) {
        let mut diagnostic =
            Diagnostic::new(Rule::ArrayTypeStyle, bracket.start_byte(), "Array brackets should follow the type");
        if let Some(fix) = &fix {
            diagnostic = diagnostic.with_fix(fix.clone());
        }
        diagnostics.push(diagnostic);
    }
}

/// The column of the node's start, in characters.
fn column(node: &Node, text: &str) -> usize {
    let line_start = text[..node.start_byte()].rfind('\n').map_or(0, |i| i + 1);
    text[line_start..node.start_byte()].chars().count()
}
