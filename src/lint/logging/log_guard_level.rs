//! L009: a log guard checks another level than the log calls it guards, such as
//! `if (log.isDebugEnabled()) { log.info(...); }` (IntelliJ LoggingConditionDisagreesWithLogLevelStatement, Baseline
//! Slf4jLevelCheck): the message is dropped although its level is enabled, or the guard does not save the work.
//!
//! Checks an `if` without `else` whose condition is a guard, alone or as one operand of `&&` (as Baseline):
//! `isDebugEnabled()` and the like of SLF4J and Log4j 2 loggers, or `isLoggable(Level.FINE)` of `java.util.logging`
//! (see `shared::logger_api`). The guarded calls are the level methods (`info`, ...) on the same logger variable in
//! the `then` branch, outside catch blocks (they may log failures on purpose), lambdas and class bodies (they run
//! later). Unlike Baseline, which takes the most severe call, it reports only if all guarded calls have the same
//! level, so that the right guard is certain.
//!
//! Unsafe fix: changes the guard to the calls' level. The guard may have been the intended level, the call's wrong.

use crate::lint::logging::shared::{Api, log_call, logger_api, names_type};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::If { cond, then, else_: None } = &tree.kind else { return };
    let Some(guard) = guard_in(cond, &context.ancestors, context) else { return };
    let mut ancestors = context.ancestors.clone();
    ancestors.push(tree);
    let mut levels = Vec::new();
    collect_levels(then, &mut ancestors, guard.api, guard.receiver, context, &mut levels);
    let Some(&level) = levels.first() else { return };
    if levels.iter().any(|&l| l != level) || level == guard.level {
        return;
    }
    let levels = guard.api.levels();
    let message = format!("guard checks level {} for {} calls", levels[guard.level], levels[level]);
    let replacement = match guard.api {
        Api::Jul => levels[level].to_uppercase(),
        Api::Slf4j | Api::Log4j2 => format!("is{}Enabled", capitalized(levels[level])),
    };
    let fix = vec![Edit::new(guard.name.clone(), replacement)];
    diagnostics.push(Diagnostic::new(Rule::LogGuardLevel, tree.start, message).with_unsafe_fix(fix));
}

/// A level check on a logger.
struct Guard<'t> {
    api: Api,
    receiver: &'t str,
    /// The index of the level in [`Api::levels`].
    level: usize,
    /// The range of the name that tells the level: the method's (`isDebugEnabled`) or the constant's (`FINE`).
    name: std::ops::Range<usize>,
}

/// The guard in the condition: the condition itself, or the only guard among the operands of `&&`.
fn guard_in<'t>(cond: &'t Tree, ancestors: &[&'t Tree], context: &Context) -> Option<Guard<'t>> {
    let cond = strip_parens(cond);
    match &cond.kind {
        K::Binary { op, lhs, rhs } if op == "&&" => {
            match (guard_in(lhs, ancestors, context), guard_in(rhs, ancestors, context)) {
                (Some(guard), None) | (None, Some(guard)) => Some(guard),
                _ => None,
            }
        }
        K::MethodInvocation { select, args, .. } => {
            let K::MemberSelect { expr: receiver, name } = &select.kind else { return None };
            let K::Ident(receiver_name) = &receiver.kind else { return None };
            let api = logger_api(receiver, ancestors, context)?;
            let name_range = context.tokens().previous(select.end)?;
            let (level, name) = match (api, args.as_slice()) {
                (Api::Jul, [argument]) if name == "isLoggable" => jul_level(argument, ancestors, context)?,
                (Api::Slf4j | Api::Log4j2, []) => {
                    let level = name.strip_prefix("is")?.strip_suffix("Enabled")?.to_lowercase();
                    (api.levels().iter().position(|l| *l == level)?, name_range)
                }
                _ => return None,
            };
            Some(Guard { api, receiver: receiver_name, level, name })
        }
        _ => None,
    }
}

/// The level of `Level.FINE` and the like, with the range of the constant's name.
fn jul_level(argument: &Tree, ancestors: &[&Tree], context: &Context) -> Option<(usize, std::ops::Range<usize>)> {
    let K::MemberSelect { expr, name } = &argument.kind else { return None };
    let unit = ancestors.first()?;
    if !names_type(expr, "java.util.logging.Level", unit) {
        return None;
    }
    let level = Api::Jul.levels().iter().position(|l| l.to_uppercase() == *name)?;
    Some((level, context.tokens().previous(argument.end)?))
}

/// Adds the levels of the calls of level methods on the logger `receiver` in `tree` (below `ancestors`), except in
/// catch blocks, lambdas and class bodies.
fn collect_levels<'t>(
    tree: &'t Tree,
    ancestors: &mut Vec<&'t Tree>,
    api: Api,
    receiver: &str,
    context: &Context,
    levels: &mut Vec<usize>,
) {
    match &tree.kind {
        K::Lambda { .. } | K::Class(_) | K::NewClass { body: Some(_), .. } => return,
        K::MethodInvocation { select, args, .. } => {
            if let Some(call) = log_call(tree, ancestors, context)
                && call.api == api
                && call.receiver == receiver
            {
                levels.push(call.level);
            }
            // `log.log(Level.INFO, ...)` of java.util.logging.
            if api == Api::Jul
                && let K::MemberSelect { expr, name } = &select.kind
                && matches!(&expr.kind, K::Ident(r) if r == receiver)
                && name == "log"
                && let Some((level, _)) = args.first().and_then(|first| jul_level(first, ancestors, context))
            {
                levels.push(level);
            }
        }
        _ => {}
    }
    ancestors.push(tree);
    let catches: Vec<&Tree> = match &tree.kind {
        K::Try { catches, .. } => catches.iter().map(|c| &*c.block).collect(),
        _ => Vec::new(),
    };
    tree.for_each_child(&mut |child| {
        if !catches.iter().any(|c| std::ptr::eq(*c, child)) {
            collect_levels(child, ancestors, api, receiver, context, levels);
        }
    });
    ancestors.pop();
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map(|c| c.to_ascii_uppercase().to_string() + chars.as_str()).unwrap_or_default()
}
