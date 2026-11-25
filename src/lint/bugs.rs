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
mod shared_b;
pub(super) mod string_literal_equality;
pub(super) mod unused_local_variable;
mod unused_private_member;

mod print_stack_trace;
mod shared_a;
mod suspicious_octal_escape;
mod unused_parameter;

mod compare_to_constant;
mod constant_bitmask;
mod constant_comparison;
mod constant_overflow;
mod division_by_zero;
mod nan_comparison;
mod oddness_check;
mod random_cast_to_int;
mod shared_arithmetic;
mod xor_power;

mod collection_with_itself;
mod equals_self_call;
mod exception_not_thrown;
mod invalid_regex;
mod label_in_switch;
mod regex_dot;
mod serial_version_uid;
mod shared_misc;
mod string_builder_char;
mod suspicious_date_pattern;
mod throw_null;

use super::Rule;

use jtk_parser::ast::Tag;

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
    (Rule::CompareToConstant, &[Tag::Binary], compare_to_constant::check),
    (Rule::NanComparison, &[Tag::Binary], nan_comparison::check),
    (Rule::ConstantBitmask, &[Tag::Binary], constant_bitmask::check),
    (Rule::ConstantComparison, &[Tag::Binary], constant_comparison::check),
    (Rule::OddnessCheck, &[Tag::Binary], oddness_check::check),
    (Rule::ConstantOverflow, &[Tag::Binary, Tag::Unary, Tag::Cast], constant_overflow::check),
    (Rule::DivisionByZero, &[Tag::Binary, Tag::Assign], division_by_zero::check),
    (Rule::RandomCastToInt, &[Tag::Cast], random_cast_to_int::check),
    (Rule::XorPower, &[Tag::Binary], xor_power::check),
    (Rule::EqualsSelfCall, &[Tag::Method], equals_self_call::check),
    (Rule::StringBuilderChar, &[Tag::NewClass], string_builder_char::check),
    (Rule::RegexDot, &[Tag::MethodInvocation], regex_dot::check),
    (Rule::InvalidRegex, &[Tag::MethodInvocation], invalid_regex::check),
    (Rule::SuspiciousDatePattern, &[Tag::MethodInvocation, Tag::NewClass], suspicious_date_pattern::check),
    (Rule::CollectionWithItself, &[Tag::MethodInvocation], collection_with_itself::check),
    (Rule::LabelInSwitch, &[Tag::Labeled], label_in_switch::check),
    (Rule::ExceptionNotThrown, &[Tag::ExprStmt], exception_not_thrown::check),
    (Rule::SerialVersionUid, &[Tag::Vars], serial_version_uid::check),
    (Rule::ThrowNull, &[Tag::Throw], throw_null::check),
];
