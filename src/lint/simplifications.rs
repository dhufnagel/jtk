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

use std::ops::Range;

use super::{Edit, Rule};

use crate::ast::Tag;

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], super::tree::TreeCheck)] = &[
    (Rule::MissingBraces, &[Tag::If, Tag::For, Tag::ForEach, Tag::While, Tag::DoWhile], missing_braces::check),
    (Rule::SimplifyBooleanReturn, &[Tag::If], simplify_boolean_return::check),
    (Rule::SimplifyBooleanExpression, &[Tag::Binary, Tag::Unary, Tag::Conditional], simplify_boolean_expression::check),
    (Rule::CollapsibleIf, &[Tag::If], collapsible_if::check),
    (Rule::LocalBeforeReturn, &[Tag::Return], local_before_return::check),
    (Rule::UnnecessaryParentheses, &[Tag::Parens, Tag::Lambda], unnecessary_parentheses::check),
    (
        Rule::UnnecessarySemicolon,
        &[
            Tag::CompilationUnit,
            Tag::Class,
            Tag::NewClass,
            Tag::Try,
            Tag::Block,
            Tag::Switch,
            Tag::SwitchExpr,
            Tag::If,
            Tag::While,
            Tag::For,
            Tag::ForEach,
            Tag::Labeled,
            Tag::DoWhile,
        ],
        unnecessary_semicolon::check,
    ),
    (Rule::RedundantModifier, &[Tag::Class, Tag::Method, Tag::Vars, Tag::Try, Tag::Lambda], redundant_modifier::check),
    (Rule::AnnotationAfterModifier, &[Tag::Class, Tag::Method, Tag::Try], annotation_after_modifier::check),
    (Rule::UnnecessaryReturn, &[Tag::Return], unnecessary_return::check),
    (Rule::SuperConstructorCall, &[Tag::Method], super_constructor_call::check),
    (Rule::ExplicitInitialization, &[Tag::Vars], explicit_initialization::check),
    (
        Rule::MultipleDeclarations,
        &[Tag::Block, Tag::Switch, Tag::SwitchExpr, Tag::Class, Tag::NewClass],
        multiple_declarations::check,
    ),
    (
        Rule::ArrayTypeStyle,
        &[Tag::Vars, Tag::Method, Tag::Class, Tag::Lambda, Tag::Try, Tag::ForEach],
        array_type_style::check,
    ),
    (Rule::LiteralCase, &[Tag::Literal], literal_case::check),
    (
        Rule::EmptyBlock,
        &[
            Tag::If,
            Tag::While,
            Tag::DoWhile,
            Tag::For,
            Tag::ForEach,
            Tag::Synchronized,
            Tag::Try,
            Tag::Class,
            Tag::NewClass,
            Tag::Switch,
            Tag::SwitchExpr,
        ],
        empty_block::check,
    ),
    (Rule::NestedBlock, &[Tag::Block, Tag::Switch, Tag::SwitchExpr], nested_block::check),
    (Rule::UnnecessaryInstantiation, &[Tag::NewClass, Tag::MethodInvocation], unnecessary_instantiation::check),
    (Rule::NullCheckBeforeInstanceof, &[Tag::InstanceOf], null_check_before_instanceof::check),
    (Rule::ExpressionLambda, &[Tag::Lambda], expression_lambda::check),
];

/// The range that removes a token or annotation together with the whitespace after it on the same line.
fn removal_range(range: Range<usize>, text: &str) -> Range<usize> {
    let trailing = text[range.end..].len() - text[range.end..].trim_start_matches([' ', '\t']).len();
    range.start..range.end + trailing
}

/// The edit that removes a statement: its whole line if nothing else is on it, otherwise with the spaces after it.
fn line_removal(range: Range<usize>, text: &str) -> Edit {
    Edit::new(super::tree::span_removal_range(range, text), "")
}
