//! S009: annotations after modifier keywords (the annotation part of Checkstyle ModifierOrder). The order of the
//! keywords themselves is left to the formatter.
//!
//! Like Checkstyle, the check stops at the first keyword out of the JLS order, and allows annotations after the
//! keywords where they may annotate the type: on fields, variables, parameters, constructors and non-void methods.
//! The fix moves the annotations before the first keyword (annotations in the modifier list annotate the same thing
//! wherever they are).

use tree_sitter::Node;

use super::contains_comment;
use crate::lint::syntax::{modifiers, named_children, removal_range, source};
use crate::lint::{Diagnostic, Edit, Rule};

const JLS_ORDER: &[&str] = &[
    "public",
    "protected",
    "private",
    "abstract",
    "default",
    "static",
    "sealed",
    "non-sealed",
    "final",
    "transient",
    "volatile",
    "synchronized",
    "native",
    "strictfp",
];

pub fn check(declaration: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if may_annotate_type(declaration) {
        return;
    }
    let Some(modifiers) = modifiers(declaration) else { return };
    let items = named_children(&modifiers);
    let mut first_keyword = None;
    let mut last_rank = 0;
    for (index, item) in items.iter().enumerate() {
        if is_annotation(item) {
            if let Some(first_keyword) = first_keyword {
                report(&modifiers, &items, first_keyword, item, text, diagnostics);
                return;
            }
        } else if let Some(rank) = JLS_ORDER.iter().position(|k| *k == source(item, text)) {
            // Annotations after misordered keywords are left until the formatter has sorted the keywords.
            if rank < last_rank {
                return;
            }
            last_rank = rank;
            first_keyword.get_or_insert(index);
        }
    }
}

/// Whether an annotation after the modifier keywords may be meant for the declared type rather than the declaration
/// (`public @Nullable String name()`).
fn may_annotate_type(declaration: &Node) -> bool {
    match declaration.kind() {
        "method_declaration" => declaration.child_by_field_name("type").is_some_and(|t| t.kind() != "void_type"),
        "field_declaration"
        | "constant_declaration"
        | "local_variable_declaration"
        | "formal_parameter"
        | "spread_parameter"
        | "catch_formal_parameter"
        | "enhanced_for_statement"
        | "constructor_declaration" => true,
        _ => false,
    }
}

fn is_annotation(node: &Node) -> bool {
    matches!(node.kind(), "annotation" | "marker_annotation")
}

fn report(
    modifiers: &Node,
    items: &[Node],
    first_keyword: usize,
    annotation: &Node,
    text: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let name = annotation.child_by_field_name("name").map_or("", |n| source(&n, text));
    let message = format!("'@{name}' annotation should precede the modifier keywords");
    let mut diagnostic = Diagnostic::new(Rule::AnnotationAfterModifier, annotation.start_byte(), message);
    if !contains_comment(modifiers) {
        let misplaced: Vec<&Node> = items[first_keyword..].iter().filter(|m| is_annotation(m)).collect();
        let moved: String = misplaced.iter().map(|a| format!("{} ", source(a, text))).collect();
        let insertion = items[first_keyword].start_byte();
        let mut edits = vec![Edit::new(insertion..insertion, moved)];
        edits.extend(misplaced.iter().map(|a| Edit::new(removal_range(a, text), "")));
        diagnostic = diagnostic.with_fix(edits);
    }
    diagnostics.push(diagnostic);
}
