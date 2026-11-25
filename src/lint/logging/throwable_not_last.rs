//! L006: an exception passed to a log call before other arguments, such as `log.warn("x {}", e, id)` (Baseline
//! Slf4jThrowable): SLF4J and Log4j 2 only log the stack trace of a throwable that comes last.
//!
//! Checks the level methods (`info`, ...) of SLF4J and Log4j 2 loggers (see `shared::logger_api`); only arguments
//! locally known to be throwables count (see `shared::kind`): catch parameters, `new XException(...)`, variables of a
//! type named `...Exception`. Safe fix, as in Baseline: moves the exception to the end. It is offered only where the
//! result is surely meant: the message is a constant with one placeholder less than arguments (so the exception
//! took the place of a value), the arguments after the exception are known not to be throwables, and moving does not
//! reorder side effects (all moved arguments are names or literals) or comments.

use crate::lint::logging::shared::{Api, Kind, constant_string, count_placeholders, kind, log_call};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(call) = log_call(tree, &context.ancestors, context) else { return };
    if call.api == Api::Jul || call.args.len() < 3 {
        return;
    }
    let mut ancestors = context.ancestors.clone();
    ancestors.push(tree);
    let kinds: Vec<Kind> = call.args.iter().map(|a| kind(a, &ancestors, context)).collect();
    let last = kinds.len() - 1;
    let Some(index) = (1..last).find(|&i| kinds[i] == Kind::Throwable) else { return };
    let throwable = &call.args[index];
    let diagnostic = Diagnostic::new(Rule::ThrowableNotLast, throwable.start, "exception is not the last argument");
    let later = &call.args[index + 1..];
    let placeholders = constant_string(&call.args[0], context.text).and_then(|m| count_placeholders(&m));
    let fixable = placeholders == Some(call.args.len() - 2)
        && kinds[index + 1..].iter().all(|k| matches!(k, Kind::Other { .. }))
        && call.args[index..].iter().all(is_name_or_literal)
        && !context.tokens().has_comment(throwable.start..call.args[last].end);
    if !fixable {
        diagnostics.push(diagnostic);
        return;
    }
    // Each argument from the exception on takes the place of the one before, and the exception the last place.
    let source = |t: &Tree| context.source(t).to_string();
    let mut edits: Vec<Edit> =
        later.iter().zip(&call.args[index..]).map(|(from, to)| Edit::new(to.start..to.end, source(from))).collect();
    edits.push(Edit::new(call.args[last].start..call.args[last].end, source(throwable)));
    diagnostics.push(diagnostic.with_fix(edits));
}

/// Whether the expression is a name, a qualified name or a literal, whose evaluation has no side effects.
fn is_name_or_literal(tree: &Tree) -> bool {
    match &strip_parens(tree).kind {
        K::Ident(_) | K::Literal(_) => true,
        K::MemberSelect { expr, .. } => is_name_or_literal(expr),
        _ => false,
    }
}
