//! Rules of group A: design and structure of types and files.

mod constant_interface;
mod declaration_usage_distance;
mod final_class;
mod magic_number;
mod mutable_exception;
mod one_top_level_class;
mod outer_type_filename;
mod overload_order;
mod package_directory;
mod utility_class_constructor;
mod visibility_modifier;

mod abstract_without_abstract;
mod throws_exception;

use tree_sitter::Node;

use super::syntax::{is_comment, modifiers, named_children, source};
use super::{Check, ConfiguredCheck, FileCheck, Rule};

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[crate::ast::Tag], super::tree::TreeCheck)] = &[];

pub(super) const CHECKS: &[(Rule, &[&str], Check)] = &[
    (Rule::UtilityClassConstructor, &["class_declaration"], utility_class_constructor::check),
    (Rule::FinalClass, &["class_declaration"], final_class::check),
    (Rule::VisibilityModifier, &["field_declaration"], visibility_modifier::check),
    (Rule::MutableException, &["class_declaration"], mutable_exception::check),
    (Rule::ConstantInterface, &["interface_declaration"], constant_interface::check),
    (Rule::OneTopLevelClass, &["program"], one_top_level_class::check),
    (
        Rule::OverloadOrder,
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "object_creation_expression",
        ],
        overload_order::check,
    ),
    (Rule::ThrowsException, &["method_declaration", "constructor_declaration"], throws_exception::check),
    (Rule::AbstractWithoutAbstract, &["class_declaration"], abstract_without_abstract::check),
];

/// The checks of rules with parameters.
pub(super) const CONFIGURED_CHECKS: &[(Rule, &[&str], ConfiguredCheck)] = &[
    (
        Rule::DeclarationUsageDistance,
        &["block", "constructor_body", "switch_block_statement_group"],
        declaration_usage_distance::check,
    ),
    (Rule::MagicNumber, &["program"], magic_number::check),
];

/// Checks of the compilation unit against the file's path.
pub(super) const FILE_CHECKS: &[(Rule, FileCheck)] =
    &[(Rule::PackageDirectory, package_directory::check), (Rule::OuterTypeFilename, outer_type_filename::check)];

/// The declarations of named types (Checkstyle's `CLASS_DEF`, `INTERFACE_DEF`, ...).
const TYPE_DECLARATIONS: &[&str] = &[
    "class_declaration",
    "interface_declaration",
    "enum_declaration",
    "annotation_type_declaration",
    "record_declaration",
];

fn is_type_declaration(node: &Node) -> bool {
    TYPE_DECLARATIONS.contains(&node.kind())
}

/// The members of a type body, without comments; an enum's members come after its constants.
fn members<'t>(body: &Node<'t>) -> Vec<Node<'t>> {
    let mut members = Vec::new();
    for child in named_children(body) {
        match child.kind() {
            "enum_body_declarations" => members.extend(named_children(&child).into_iter().filter(|c| !is_comment(c))),
            _ if is_comment(&child) => {}
            _ => members.push(child),
        }
    }
    members
}

/// The members of a type body like [`members`], with the stray semicolons Checkstyle keeps as members too.
fn members_and_semicolons<'t>(body: &Node<'t>) -> Vec<Node<'t>> {
    let mut members = Vec::new();
    let mut add = |node: &Node<'t>| {
        for child in node.children(&mut node.walk()) {
            if (child.is_named() && !is_comment(&child) && child.kind() != "enum_body_declarations")
                || child.kind() == ";"
            {
                members.push(child);
            }
        }
    };
    add(body);
    if let Some(declarations) = named_children(body).into_iter().find(|c| c.kind() == "enum_body_declarations") {
        add(&declarations);
    }
    members
}

/// The body of a type declaration or anonymous class.
fn body<'t>(declaration: &Node<'t>) -> Option<Node<'t>> {
    declaration
        .child_by_field_name("body")
        .or_else(|| named_children(declaration).into_iter().find(|c| c.kind() == "class_body"))
}

/// Whether the declaration's modifiers include the keyword (`static`, `final`, ...), without annotations.
fn has_keyword(declaration: &Node, text: &str, keyword: &str) -> bool {
    modifiers(declaration).is_some_and(|m| {
        m.children(&mut m.walk())
            .any(|c| !matches!(c.kind(), "annotation" | "marker_annotation") && source(&c, text) == keyword)
    })
}

/// The names of the declarators of a field or variable declaration.
fn declarator_names<'t>(declaration: &Node<'t>) -> Vec<Node<'t>> {
    named_children(declaration)
        .into_iter()
        .filter(|d| d.kind() == "variable_declarator")
        .filter_map(|d| d.child_by_field_name("name"))
        .collect()
}

/// The text of a (qualified) name without whitespace and comments: `a . b` is `a.b`.
fn qualified_name(node: &Node, text: &str) -> String {
    let mut parts = Vec::new();
    super::syntax::for_each_descendant(node, &["type_arguments", "annotation", "marker_annotation"], |n| {
        if matches!(n.kind(), "identifier" | "type_identifier") {
            parts.push(source(n, text));
        }
    });
    if parts.is_empty() { source(node, text).to_string() } else { parts.join(".") }
}

/// The package a compilation unit declares, or `""`.
fn package_name(program: &Node, text: &str) -> String {
    named_children(program)
        .into_iter()
        .find(|c| c.kind() == "package_declaration")
        .and_then(|p| named_children(&p).into_iter().find(|c| matches!(c.kind(), "scoped_identifier" | "identifier")))
        .map_or(String::new(), |name| qualified_name(&name, text))
}

/// Calls `visit` when entering and when leaving each node of the tree in document order. Iterative, so that deeply
/// nested code cannot overflow the stack.
fn walk<'t>(root: &Node<'t>, mut visit: impl FnMut(&Node<'t>, bool)) {
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        visit(&node, true);
        if cursor.goto_first_child() {
            continue;
        }
        visit(&node, false);
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return;
            }
            visit(&cursor.node(), false);
        }
    }
}

/// The root of the tree that holds the node. Expensive (`Node::parent` searches from the root), so only for rare
/// cases.
fn root<'t>(node: &Node<'t>) -> Node<'t> {
    let mut root = *node;
    while let Some(parent) = root.parent() {
        root = parent;
    }
    root
}
