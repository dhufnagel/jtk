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
mod shared_b;
mod text_block;
mod unnamed_variable;
mod use_var;
mod when_guard;

use super::Rule;
use crate::ast::Tag;

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], super::tree::TreeCheck)] = &[
    (Rule::InstanceofPattern, &[Tag::If], instanceof_pattern::check),
    (Rule::DiamondOperator, &[Tag::Vars, Tag::Assign, Tag::Return], diamond_operator::check),
    (Rule::EnhancedSwitch, &[Tag::Switch, Tag::SwitchExpr], enhanced_switch::check),
    (Rule::WhenGuard, &[Tag::Switch, Tag::SwitchExpr], when_guard::check),
    (Rule::RecordPatternTypeArguments, &[Tag::RecordPattern], record_pattern_type_arguments::check),
    (Rule::UnnamedVariable, &[Tag::Try], unnamed_variable::check_try),
    (Rule::UnnamedVariable, &[Tag::Lambda], unnamed_variable::check_lambda),
    (Rule::MissingDeprecated, &[Tag::CompilationUnit, Tag::Class, Tag::NewClass], missing_deprecated::check),
    (Rule::SealedPermits, &[Tag::Class], sealed_permits::missing_permits),
    (Rule::UnnecessaryPermits, &[Tag::CompilationUnit], sealed_permits::unnecessary_permits),
    (Rule::TextBlock, &[Tag::Binary], text_block::check),
    (Rule::IdenticalCatch, &[Tag::Try], identical_catch::check),
    (Rule::FinalParameters, &[Tag::Method], final_parameters::check),
    (Rule::FinalLocalVariable, &[Tag::CompilationUnit], final_local_variable::check),
    (Rule::UseVar, &[Tag::Vars], use_var::check),
    (Rule::EnhancedFor, &[Tag::For], enhanced_for::check),
    (Rule::FinalField, &[Tag::CompilationUnit], final_field::check),
];
