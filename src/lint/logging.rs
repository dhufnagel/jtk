//! Rules of group L: logging calls.

mod foreign_logger_class;
mod log_guard_level;
mod placeholder_count;
pub(super) mod shared;
mod throwable_not_last;

// Batch 7.
mod concatenated_log_message;

use super::Rule;

use jtk_parser::ast::Tag;

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], super::tree::TreeCheck)] = &[
    (Rule::ForeignLoggerClass, &[Tag::Vars], foreign_logger_class::check),
    (Rule::PlaceholderCount, &[Tag::MethodInvocation], placeholder_count::check),
    (Rule::ThrowableNotLast, &[Tag::MethodInvocation], throwable_not_last::check),
    (Rule::LogGuardLevel, &[Tag::If], log_guard_level::check),
    // Batch 7.
    (Rule::ConcatenatedLogMessage, &[Tag::MethodInvocation], concatenated_log_message::check),
];
