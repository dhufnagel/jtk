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
mod text_block;
mod unnamed_variable;
mod use_var;
mod when_guard;

use super::{Check, Rule};

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
    (Rule::FinalParameters, &["method_declaration", "constructor_declaration"], final_parameters::check),
    (
        Rule::MissingDeprecated,
        &["program", "class_body", "interface_body", "enum_body", "enum_body_declarations", "annotation_type_body"],
        missing_deprecated::check,
    ),
    (Rule::SealedPermits, &["class_declaration", "interface_declaration"], sealed_permits::missing_permits),
    (Rule::FinalLocalVariable, &["program"], final_local_variable::check),
    (Rule::UnnecessaryPermits, &["program"], sealed_permits::unnecessary_permits),
    (Rule::UseVar, &["local_variable_declaration"], use_var::check),
    (Rule::EnhancedFor, &["for_statement"], enhanced_for::check),
    (Rule::TextBlock, &["program", "binary_expression"], text_block::check),
    (Rule::IdenticalCatch, &["try_statement", "try_with_resources_statement"], identical_catch::check),
    (Rule::FinalField, &["program"], final_field::check),
];
