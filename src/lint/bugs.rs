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
mod fall_through_cst;
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
mod shared_b;
pub(super) mod string_literal_equality;
pub(super) mod unused_local_variable;
mod unused_private_member;

mod print_stack_trace;
mod shared_a;
mod suspicious_octal_escape;
mod unused_parameter;

use super::{Check, ConfiguredCheck, Rule};

use crate::ast::Tag;

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], super::tree::TreeCheck)] = &[
    (Rule::EmptyCatch, &[Tag::Try], empty_catch::check),
    (Rule::StringLiteralEquality, &[Tag::Binary], string_literal_equality::check),
    (Rule::UnusedLocalVariable, &[Tag::Vars, Tag::ForEach, Tag::BindingPattern], unused_local_variable::check),
    (Rule::EqualsHashCode, &[Tag::Class, Tag::NewClass], equals_hash_code::check),
    (Rule::CovariantEquals, &[Tag::Class, Tag::NewClass], covariant_equals::check),
    (Rule::FallThrough, &[Tag::Switch, Tag::SwitchExpr], fall_through::check),
    (Rule::MissingSwitchDefault, &[Tag::Switch], missing_switch_default::check),
    (Rule::DefaultComesLast, &[Tag::Switch, Tag::SwitchExpr], default_comes_last::check),
    (Rule::EqualsAvoidNull, &[Tag::MethodInvocation], equals_avoid_null::check),
    (Rule::EqualsNull, &[Tag::MethodInvocation], equals_null::check),
    (Rule::FinallyExit, &[Tag::Try], finally_exit::check),
    (Rule::IdenticalOperands, &[Tag::Binary, Tag::Assign], identical_operands::check),
    (Rule::InnerAssignment, &[Tag::Assign], inner_assignment::check),
    (Rule::ParameterAssignment, &[Tag::Method, Tag::Lambda], parameter_assignment::check),
    (Rule::ModifiedControlVariable, &[Tag::For, Tag::ForEach], modified_control_variable::check),
    (Rule::JumbledIncrementer, &[Tag::For], jumbled_incrementer::check),
    (Rule::NullCheck, &[Tag::Binary], null_check::check),
    (Rule::BigDecimalDouble, &[Tag::NewClass], big_decimal_double::check),
    (Rule::SuspiciousOctalEscape, &[Tag::Literal], suspicious_octal_escape::check),
    (Rule::DoubleCheckedLocking, &[Tag::If], double_checked_locking::check),
    (Rule::IllegalCatch, &[Tag::Try], illegal_catch::check),
    (Rule::IllegalThrows, &[Tag::Method], illegal_throws::check),
    (Rule::PrintStackTrace, &[Tag::MethodInvocation], print_stack_trace::check),
    (Rule::LostCause, &[Tag::Throw], lost_cause::check),
    (Rule::MissingSuperCall, &[Tag::Method], missing_super_call::check),
    (Rule::DoubleBraceInitialization, &[Tag::NewClass], double_brace_initialization::check),
    (Rule::UnusedPrivateMember, &[Tag::CompilationUnit], unused_private_member::check),
    (Rule::UnusedParameter, &[Tag::Method], unused_parameter::check),
    (Rule::PatternVariableAssignment, &[Tag::If, Tag::Conditional], pattern_variable_assignment::check),
    (Rule::MissingNullCase, &[Tag::Switch, Tag::SwitchExpr], missing_null_case::check),
    (Rule::Finalizer, &[Tag::Method], avoided_methods::check_finalizer),
    (Rule::Clone, &[Tag::Method], avoided_methods::check_clone),
];

pub(super) const CHECKS: &[(Rule, &[&str], Check)] = &[
];

/// The checks of rules with parameters.
pub(super) const CONFIGURED_CHECKS: &[(Rule, &[&str], ConfiguredCheck)] = &[];
