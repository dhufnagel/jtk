//! Rules of group K: concurrency.

mod interrupted_in_catch;
mod lock_without_try;
mod non_atomic_volatile;
mod shared;
mod static_field_instance_lock;
mod static_formatter;
mod synchronize_on_non_final;
mod synchronize_on_value_based;
mod thread_not_started;
mod thread_run;
mod thread_stop;
mod wait_not_in_loop;
mod wait_on_condition;
mod zero_core_pool;

use super::Rule;

use jtk_parser::ast::Tag;

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], super::tree::TreeCheck)] = &[
    (Rule::ThreadRun, &[Tag::MethodInvocation], thread_run::check),
    (Rule::ThreadNotStarted, &[Tag::ExprStmt], thread_not_started::check),
    (Rule::ThreadStop, &[Tag::MethodInvocation], thread_stop::check),
    (Rule::InterruptedInCatch, &[Tag::Try], interrupted_in_catch::check),
    (Rule::WaitNotInLoop, &[Tag::MethodInvocation], wait_not_in_loop::check),
    (Rule::WaitOnCondition, &[Tag::MethodInvocation], wait_on_condition::check),
    (Rule::LockWithoutTry, &[Tag::MethodInvocation], lock_without_try::check),
    (Rule::SynchronizeOnNonFinal, &[Tag::Synchronized], synchronize_on_non_final::check),
    (Rule::SynchronizeOnValueBased, &[Tag::Synchronized], synchronize_on_value_based::check),
    (Rule::StaticFieldInstanceLock, &[Tag::Method, Tag::Synchronized], static_field_instance_lock::check),
    (Rule::NonAtomicVolatile, &[Tag::Unary, Tag::Assign], non_atomic_volatile::check),
    (Rule::StaticFormatter, &[Tag::Vars], static_formatter::check),
    (Rule::ZeroCorePool, &[Tag::NewClass, Tag::MethodInvocation], zero_core_pool::check),
];
