//! L004: the number of `{}` placeholders in a log message differs from the number of arguments, such as
//! `log.info("a={} b={}", a)` (IntelliJ LoggingPlaceholderCountMatchesArgumentCount, Picnic Slf4jLogStatement, PMD
//! InvalidLogMessageFormat): an argument is not logged, or a placeholder is printed as is.
//!
//! No fix. Checks the level methods (`info`, ...) of SLF4J and Log4j 2 loggers (see `shared::logger_api`) whose first
//! argument is a string literal or a concatenation of them; calls with a `Marker` first are left out. A placeholder
//! escaped with a backslash (`\\{}`) does not count; messages with two backslashes before a placeholder, Unicode or
//! octal escapes are left out, as the APIs treat them differently.
//!
//! A last argument that may be a throwable may be the logged exception rather than a placeholder's value, so the
//! count may be one less; only an argument whose type is locally known not to be a throwable (see `shared::kind`)
//! must have a placeholder. A single argument may also be an `Object[]` holding the values, so a call with one
//! argument is only checked if that argument's type is locally known and no array.

use crate::lint::logging::shared::{Api, Kind, constant_string, count_placeholders, kind, log_call};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::Tree;

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(call) = log_call(tree, &context.ancestors, context) else { return };
    if call.api == Api::Jul {
        return;
    }
    let Some((message, arguments)) = call.args.split_first() else { return };
    let Some(placeholders) = constant_string(message, context.text).and_then(|m| count_placeholders(&m)) else {
        return;
    };
    let mut ancestors = context.ancestors.clone();
    ancestors.push(tree);
    let count = arguments.len();
    let last = arguments.last().map(|a| kind(a, &ancestors, context));
    let accepted = match last {
        None => placeholders == 0,
        Some(Kind::Unknown) if count == 1 => true,
        Some(Kind::Other { array: true }) if count == 1 => true,
        Some(Kind::Other { .. }) => placeholders == count,
        Some(Kind::Throwable | Kind::Unknown) => placeholders == count || placeholders + 1 == count,
    };
    if !accepted {
        let message = format!("{} but {}", plural(placeholders, "placeholder"), plural(count, "argument"));
        diagnostics.push(Diagnostic::new(Rule::PlaceholderCount, tree.start, message));
    }
}

fn plural(count: usize, noun: &str) -> String {
    match count {
        0 => format!("no {noun}s"),
        1 => format!("1 {noun}"),
        _ => format!("{count} {noun}s"),
    }
}
