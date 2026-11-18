//! Rules of group S: code that can be written more simply.

mod annotation_after_modifier;
mod array_type_style;
mod collapsible_if;
mod empty_block;
mod explicit_initialization;
mod expression_lambda;
mod literal_case;
mod local_before_return;
mod missing_braces;
mod multiple_declarations;
mod nested_block;
mod null_check_before_instanceof;
mod redundant_modifier;
mod simplify_boolean_expression;
pub(super) mod simplify_boolean_return;
mod super_constructor_call;
mod unnecessary_instantiation;
mod unnecessary_parentheses;
mod unnecessary_return;
mod unnecessary_semicolon;

use tree_sitter::Node;

use super::{Check, Edit, Rule, syntax};

use crate::ast::Tag;

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], super::tree::TreeCheck)] =
    &[(Rule::SimplifyBooleanReturn, &[Tag::If], simplify_boolean_return::check)];

pub(super) const CHECKS: &[(Rule, &[&str], Check)] = &[
    (
        Rule::MissingBraces,
        &["if_statement", "for_statement", "enhanced_for_statement", "while_statement", "do_statement"],
        missing_braces::check,
    ),
    (
        Rule::SimplifyBooleanExpression,
        &["binary_expression", "unary_expression", "ternary_expression"],
        simplify_boolean_expression::check,
    ),
    (Rule::UnnecessaryParentheses, &["parenthesized_expression", "lambda_expression"], unnecessary_parentheses::check),
    (
        Rule::UnnecessarySemicolon,
        &[
            "program",
            "class_body",
            "interface_body",
            "annotation_type_body",
            "enum_body_declarations",
            "resource_specification",
            "block",
            "constructor_body",
            "switch_block_statement_group",
            "if_statement",
            "while_statement",
            "for_statement",
            "enhanced_for_statement",
            "labeled_statement",
            "do_statement",
        ],
        unnecessary_semicolon::check,
    ),
    (
        Rule::RedundantModifier,
        &[
            "interface_declaration",
            "annotation_type_declaration",
            "enum_declaration",
            "record_declaration",
            "class_declaration",
            "method_declaration",
            "constructor_declaration",
            "field_declaration",
            "constant_declaration",
            "annotation_type_element_declaration",
            "resource",
            "local_variable_declaration",
            "catch_formal_parameter",
            "formal_parameter",
        ],
        redundant_modifier::check,
    ),
    (
        Rule::AnnotationAfterModifier,
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "record_declaration",
            "annotation_type_declaration",
            "method_declaration",
            "constructor_declaration",
            "compact_constructor_declaration",
            "field_declaration",
            "constant_declaration",
            "annotation_type_element_declaration",
            "local_variable_declaration",
            "formal_parameter",
            "spread_parameter",
            "catch_formal_parameter",
            "resource",
            "enhanced_for_statement",
        ],
        annotation_after_modifier::check,
    ),
    (Rule::SuperConstructorCall, &["constructor_body"], super_constructor_call::check),
    (Rule::ExplicitInitialization, &["field_declaration"], explicit_initialization::check),
    (Rule::MultipleDeclarations, STATEMENT_CONTAINERS, multiple_declarations::check),
    (
        Rule::ArrayTypeStyle,
        &[
            "field_declaration",
            "constant_declaration",
            "local_variable_declaration",
            "formal_parameter",
            "catch_formal_parameter",
            "resource",
            "enhanced_for_statement",
            "method_declaration",
            "annotation_type_element_declaration",
        ],
        array_type_style::check,
    ),
    (
        Rule::LiteralCase,
        &[
            "decimal_integer_literal",
            "hex_integer_literal",
            "octal_integer_literal",
            "binary_integer_literal",
            "decimal_floating_point_literal",
            "hex_floating_point_literal",
        ],
        literal_case::check,
    ),
    (
        Rule::EmptyBlock,
        &[
            "while_statement",
            "try_statement",
            "try_with_resources_statement",
            "finally_clause",
            "do_statement",
            "if_statement",
            "for_statement",
            "enhanced_for_statement",
            "static_initializer",
            "class_body",
            "enum_body_declarations",
            "switch_expression",
            "synchronized_statement",
        ],
        empty_block::check,
    ),
    (Rule::NestedBlock, &["block", "constructor_body", "switch_block_statement_group"], nested_block::check),
    (Rule::NullCheckBeforeInstanceof, &["instanceof_expression"], null_check_before_instanceof::check),
    (Rule::ExpressionLambda, &["lambda_expression"], expression_lambda::check),
    (Rule::CollapsibleIf, &["if_statement"], collapsible_if::check),
    (Rule::LocalBeforeReturn, &["return_statement"], local_before_return::check),
    (Rule::UnnecessaryReturn, &["return_statement"], unnecessary_return::check),
    (
        Rule::UnnecessaryInstantiation,
        &["object_creation_expression", "method_invocation"],
        unnecessary_instantiation::check,
    ),
];

/// The nodes whose children are member declarations or statements.
const STATEMENT_CONTAINERS: &[&str] = &[
    "class_body",
    "interface_body",
    "annotation_type_body",
    "enum_body_declarations",
    "block",
    "constructor_body",
    "switch_block_statement_group",
];

/// Whether a comment is inside the node: fixes that rewrite the node would lose it.
fn contains_comment(node: &Node) -> bool {
    comment_in(node, node.byte_range())
}

/// Whether a comment inside the node overlaps the range.
fn comment_in(node: &Node, range: std::ops::Range<usize>) -> bool {
    let mut found = false;
    syntax::for_each_descendant(node, &[], |n| {
        found |= syntax::is_comment(n) && n.start_byte() < range.end && range.start < n.end_byte();
    });
    found
}

/// The edit that removes a node: its whole line if nothing else is on it, otherwise with the spaces after it.
fn line_removal_range(node: &Node, text: &str) -> Edit {
    Edit::new(syntax::line_removal_range(node, text), "")
}
