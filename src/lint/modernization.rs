//! Rules of group M: code that newer Java versions can express better.

mod diamond_operator;
mod enhanced_for;
mod enhanced_switch;
mod final_field;
mod final_local_variable;
mod final_parameters;
mod identical_catch;
mod instanceof_pattern;
mod missing_deprecated;
mod record_pattern_type_arguments;
mod sealed_permits;
mod shared_a;
mod text_block;
mod unnamed_variable;
mod use_var;
mod when_guard;

use super::{Check, Rule};

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[crate::ast::Tag], super::tree::TreeCheck)] = &[
    (Rule::FinalParameters, &[crate::ast::Tag::Method], final_parameters::check),
    (Rule::FinalLocalVariable, &[crate::ast::Tag::CompilationUnit], final_local_variable::check),
    (Rule::UseVar, &[crate::ast::Tag::Vars], use_var::check),
    (Rule::EnhancedFor, &[crate::ast::Tag::For], enhanced_for::check),
    (Rule::FinalField, &[crate::ast::Tag::CompilationUnit], final_field::check),
];

pub(super) const CHECKS: &[(Rule, &[&str], Check)] = &[
    (Rule::InstanceofPattern, &["if_statement"], instanceof_pattern::check),
    (
        Rule::DiamondOperator,
        &[
            "local_variable_declaration",
            "field_declaration",
            "constant_declaration",
            "assignment_expression",
            "return_statement",
        ],
        diamond_operator::check,
    ),
    (Rule::EnhancedSwitch, &["switch_expression"], enhanced_switch::check),
    (Rule::WhenGuard, &["switch_rule"], when_guard::check),
    (Rule::RecordPatternTypeArguments, &["record_pattern"], record_pattern_type_arguments::check),
    (Rule::UnnamedVariable, &["catch_clause"], unnamed_variable::catch_parameter),
    (Rule::UnnamedVariable, &["lambda_expression"], unnamed_variable::lambda_parameter),
    (Rule::UnnamedVariable, &["try_with_resources_statement"], unnamed_variable::try_resource),
    (
        Rule::MissingDeprecated,
        &["program", "class_body", "interface_body", "enum_body", "enum_body_declarations", "annotation_type_body"],
        missing_deprecated::check,
    ),
    (Rule::SealedPermits, &["class_declaration", "interface_declaration"], sealed_permits::missing_permits),
    (Rule::UnnecessaryPermits, &["program"], sealed_permits::unnecessary_permits),
    (Rule::TextBlock, &["program", "binary_expression"], text_block::check),
    (Rule::IdenticalCatch, &["try_statement", "try_with_resources_statement"], identical_catch::check),
];
