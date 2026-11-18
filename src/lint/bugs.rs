//! Rules of group B: likely bugs.

mod avoided_methods;
mod big_decimal_double;
mod covariant_equals;
mod default_comes_last;
mod double_brace_initialization;
mod double_checked_locking;
pub(super) mod empty_catch;
mod equals_avoid_null;
mod equals_hash_code;
mod equals_null;
pub(super) mod fall_through;
mod finally_exit;
mod identical_operands;
mod illegal_catch;
mod illegal_throws;
mod inner_assignment;
mod jumbled_incrementer;
mod lost_cause;
mod missing_null_case;
mod missing_super_call;
mod missing_switch_default;
mod modified_control_variable;
mod null_check;
mod parameter_assignment;
mod pattern_variable_assignment;
pub(super) mod string_literal_equality;
pub(super) mod unused_local_variable;
mod unused_private_member;

mod print_stack_trace;
mod suspicious_octal_escape;
mod unused_parameter;

use super::{Check, ConfiguredCheck, Rule};

pub(super) const CHECKS: &[(Rule, &[&str], Check)] = &[
    (Rule::EqualsHashCode, &["class_body", "enum_body_declarations"], equals_hash_code::check),
    (Rule::CovariantEquals, &["class_body", "enum_body_declarations"], covariant_equals::check),
    (Rule::FallThrough, &["switch_block"], fall_through::check),
    (Rule::EqualsNull, &["method_invocation"], equals_null::check),
    (Rule::FinallyExit, &["finally_clause"], finally_exit::check),
    (Rule::IdenticalOperands, &["binary_expression", "assignment_expression"], identical_operands::check),
    (Rule::ModifiedControlVariable, &["for_statement", "enhanced_for_statement"], modified_control_variable::check),
    (Rule::JumbledIncrementer, &["for_statement"], jumbled_incrementer::check),
    (Rule::NullCheck, &["binary_expression"], null_check::check),
    (Rule::BigDecimalDouble, &["object_creation_expression"], big_decimal_double::check),
    (Rule::DoubleCheckedLocking, &["if_statement"], double_checked_locking::check),
    (Rule::UnusedPrivateMember, &["program"], unused_private_member::check),
    (
        Rule::MissingSwitchDefault,
        &[
            "block",
            "constructor_body",
            "switch_block_statement_group",
            "if_statement",
            "while_statement",
            "do_statement",
            "for_statement",
            "enhanced_for_statement",
            "labeled_statement",
        ],
        missing_switch_default::check,
    ),
    (Rule::DefaultComesLast, &["switch_block"], default_comes_last::check),
    (Rule::EqualsAvoidNull, &["method_invocation"], equals_avoid_null::check),
    (Rule::InnerAssignment, &["assignment_expression"], inner_assignment::check),
    (
        Rule::ParameterAssignment,
        &["method_declaration", "constructor_declaration", "lambda_expression"],
        parameter_assignment::check,
    ),
    (Rule::LostCause, &["throw_statement"], lost_cause::check),
    (Rule::MissingSuperCall, &["method_declaration"], missing_super_call::check),
    (Rule::DoubleBraceInitialization, &["object_creation_expression"], double_brace_initialization::check),
    (Rule::PatternVariableAssignment, &["if_statement", "ternary_expression"], pattern_variable_assignment::check),
    (Rule::MissingNullCase, &["switch_expression"], missing_null_case::check),
    (Rule::Finalizer, &["method_declaration"], avoided_methods::check_finalizer),
    (Rule::Clone, &["method_declaration"], avoided_methods::check_clone),
    (Rule::SuspiciousOctalEscape, &["string_literal"], suspicious_octal_escape::check),
    (Rule::PrintStackTrace, &["method_invocation"], print_stack_trace::check),
    (Rule::UnusedParameter, &["method_declaration", "constructor_declaration"], unused_parameter::check),
];

/// The checks of rules with parameters.
pub(super) const CONFIGURED_CHECKS: &[(Rule, &[&str], ConfiguredCheck)] = &[
    (Rule::IllegalCatch, &["catch_clause"], illegal_catch::check),
    (Rule::IllegalThrows, &["method_declaration", "constructor_declaration"], illegal_throws::check),
];
