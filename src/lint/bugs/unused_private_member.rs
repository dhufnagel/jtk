//! B027: private field or method that is never used (Checkstyle UnusedPrivateField, PMD UnusedPrivateField,
//! UnusedPrivateMethod, Error Prone UnusedVariable, UnusedMethod).
//!
//! Not reported, as they are used without being named in the code: serialization members, annotated members
//! (frameworks access them by reflection), fields in files that use Lombok, and methods named in a string (JUnit's
//! `@MethodSource("name")`).
//!
//! Unsafe fix (Eclipse save action "Remove unused private members"): removes the member with its Javadoc and the
//! comments directly above it, and a blank line that would be left doubled; from a field declaration of several
//! variables only the declarator. Fields whose value may have side effects (other than creating an object or array
//! from side-effect-free values) are kept. Unsafe because uses are found by name only, so a use through reflection
//! or a member of the same name elsewhere is not told apart.

use std::collections::HashSet;

use tree_sitter::Node;

use crate::lint::syntax::{
    for_each_descendant, for_each_descendant_with_parent, has_comment, has_modifier, is_comment, is_name_reference_in,
    is_side_effect_free, modifiers, named_children, source,
};
use crate::lint::{Diagnostic, Edit, Rule};

const SERIALIZATION_FIELDS: &[&str] = &["serialVersionUID", "serialPersistentFields"];
const SERIALIZATION_METHODS: &[&str] =
    &["writeObject", "readObject", "readObjectNoData", "writeReplace", "readResolve"];

pub fn check(program: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let names = UsedNames::find(program, text);
    let uses_lombok =
        named_children(program).iter().any(|c| c.kind() == "import_declaration" && source(c, text).contains("lombok."));
    let is_candidate = |member: &Node| has_modifier(member, text, "private") && !is_annotated(member);
    for_each_descendant(program, &[], |member| match member.kind() {
        "field_declaration" if !uses_lombok && is_candidate(member) => {
            let declarators: Vec<Node> =
                named_children(member).into_iter().filter(|d| d.kind() == "variable_declarator").collect();
            for (index, declarator) in declarators.iter().enumerate() {
                let Some(name) = declarator.child_by_field_name("name") else { continue };
                let field = source(&name, text);
                if !names.variables.contains(field) && !SERIALIZATION_FIELDS.contains(&field) {
                    let message = format!("Unused private field '{field}'");
                    let diagnostic = Diagnostic::new(Rule::UnusedPrivateMember, name.start_byte(), message);
                    diagnostics.push(match field_removal(member, &declarators, index, text) {
                        Some(edit) => diagnostic.with_unsafe_fix(vec![edit]),
                        None => diagnostic,
                    });
                }
            }
        }
        "method_declaration" if is_candidate(member) => {
            let Some(name) = member.child_by_field_name("name") else { return };
            let method = source(&name, text);
            if !names.methods.contains(method) && !SERIALIZATION_METHODS.contains(&method) {
                let message = format!("Unused private method '{method}'");
                let diagnostic = Diagnostic::new(Rule::UnusedPrivateMember, name.start_byte(), message);
                // An overload may be the method a test framework finds by name (JUnit's `@MethodSource`).
                diagnostics.push(if names.overloaded.contains(method) {
                    diagnostic
                } else {
                    diagnostic.with_unsafe_fix(vec![member_removal(member, text)])
                });
            }
        }
        _ => {}
    });
}

/// The edit that removes the declarator `index` of a field declaration, or the whole declaration if it is the only
/// one; `None` if its value may have side effects.
fn field_removal(field: &Node, declarators: &[Node], index: usize, text: &str) -> Option<Edit> {
    let declarator = declarators[index];
    if declarator.child_by_field_name("value").is_some_and(|v| !is_removable_value(&v)) {
        return None;
    }
    match (index.checked_sub(1).map(|i| declarators[i]), declarators.get(index + 1)) {
        _ if has_comment(field) => None,
        (_, Some(next)) => Some(Edit::new(declarator.start_byte()..next.start_byte(), "")),
        (Some(previous), None) => Some(Edit::new(previous.end_byte()..declarator.end_byte(), "")),
        (None, None) => Some(member_removal(field, text)),
    }
}

/// A value whose evaluation can be dropped: a side-effect-free expression, a lambda or method reference, or the
/// creation of an object or array from such values.
fn is_removable_value(value: &Node) -> bool {
    match value.kind() {
        "lambda_expression" | "method_reference" | "class_literal" => true,
        "object_creation_expression" => {
            !named_children(value).iter().any(|c| c.kind() == "class_body")
                && value.child_by_field_name("arguments").is_some_and(|a| all_removable(&a))
        }
        "array_creation_expression"
        | "array_initializer"
        | "dimensions_expr"
        | "unary_expression"
        | "binary_expression" => all_removable(value),
        _ => is_side_effect_free(value),
    }
}

fn all_removable(node: &Node) -> bool {
    named_children(node).iter().all(|c| {
        is_comment(c)
            || matches!(c.kind(), "dimensions" | "type_identifier" | "generic_type" | "integral_type")
            || is_removable_value(c)
    })
}

/// The edit that removes a member declaration with the comments directly above it (its Javadoc), and the blank line
/// after it if there is one before it too, or the blank line before it if it is the last member.
fn member_removal(member: &Node, text: &str) -> Edit {
    let mut start = member.start_byte();
    let mut previous = member.prev_sibling();
    while let Some(comment) = previous.filter(is_comment) {
        let between = &text[comment.end_byte()..start];
        let line_start = text[..comment.start_byte()].rfind('\n').map_or(0, |i| i + 1);
        let starts_line = text[line_start..comment.start_byte()].trim().is_empty();
        if !starts_line || !between.trim().is_empty() || between.matches('\n').count() > 1 {
            break;
        }
        start = comment.start_byte();
        previous = comment.prev_sibling();
    }
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[member.end_byte()..].find('\n').map_or(text.len(), |i| member.end_byte() + i + 1);
    if !text[line_start..start].trim().is_empty() || !text[member.end_byte()..line_end].trim().is_empty() {
        // Not alone on its lines.
        let trailing =
            text[member.end_byte()..].len() - text[member.end_byte()..].trim_start_matches([' ', '\t']).len();
        return Edit::new(start..member.end_byte() + trailing, "");
    }
    let blank_line_before = text[..line_start].strip_suffix('\n').is_some_and(|before| {
        let previous_line_start = before.rfind('\n').map_or(0, |i| i + 1);
        before[previous_line_start..].trim().is_empty()
    });
    let mut range = line_start..line_end;
    if blank_line_before {
        let next_line_end = text[line_end..].find('\n').map_or(text.len(), |i| line_end + i + 1);
        let next_line = &text[line_end..next_line_end];
        if next_line.trim().is_empty() && next_line_end > line_end {
            range.end = next_line_end;
        } else if next_line.trim_start().starts_with('}') {
            let before = &text[..line_start - 1];
            range.start = before.rfind('\n').map_or(0, |i| i + 1);
        }
    }
    Edit::new(range, "")
}

fn is_annotated(member: &Node) -> bool {
    modifiers(member)
        .is_some_and(|m| named_children(&m).iter().any(|c| matches!(c.kind(), "annotation" | "marker_annotation")))
}

/// The names used in the file, by name only: a use of any field or method with the name counts.
struct UsedNames<'a> {
    variables: HashSet<&'a str>,
    methods: HashSet<&'a str>,
    /// The names of methods declared more than once.
    overloaded: HashSet<&'a str>,
}

impl<'a> UsedNames<'a> {
    fn find(program: &Node, text: &'a str) -> UsedNames<'a> {
        let mut names = UsedNames { variables: HashSet::new(), methods: HashSet::new(), overloaded: HashSet::new() };
        let mut declared = HashSet::new();
        for_each_descendant_with_parent(program, |node, parent| match node.kind() {
            "identifier" if is_name_reference_in(node, parent) || parent.kind() == "field_access" => {
                names.variables.insert(source(node, text));
            }
            "identifier" if matches!(parent.kind(), "method_invocation" | "method_reference") => {
                names.methods.insert(source(node, text));
            }
            "identifier" if parent.kind() == "method_declaration" => {
                let name = source(node, text);
                if !declared.insert(name) {
                    names.overloaded.insert(name);
                }
            }
            "string_literal" => {
                names.methods.insert(source(node, text).trim_matches('"'));
            }
            _ => {}
        });
        names
    }
}
