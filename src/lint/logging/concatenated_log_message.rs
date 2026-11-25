//! L008: a log message built eagerly, by string concatenation (`log.info("x=" + x)`) or `String.format`/`formatted`,
//! instead of with placeholders (IntelliJ StringConcatenationArgumentToLogCall, NetBeans String concatenation in
//! logger, Baseline Slf4jConstantLogMessage, Picnic EagerStringFormatting, Sonar S2629/S3457): the message is built
//! even when the level is disabled, and log aggregation cannot group the messages by their pattern.
//!
//! Checks the level methods (`info`, ...) of SLF4J and Log4j 2 loggers (see `shared::logger_api`); the message is the
//! first argument, or the second after an argument locally known to be a `Marker`. A concatenation counts if it has a
//! string literal and an operand that is not a constant (a literal or an `UPPER_CASE` name); `String.format` (with
//! `String` from `java.lang`) and `"...".formatted` count if they have arguments.
//!
//! Unsafe fix: `"a " + x + " b"` becomes `"a {} b", x` and `String.format("a %s", x)` becomes `"a {}", x`, with any
//! arguments the call already had after the message (such as a trailing exception) kept after the new ones. It is
//! unsafe because the values are then formatted lazily and by the logging API: `toString()` side effects and the
//! output for arrays or `Formattable` values may differ, and a value that is a throwable as the last argument would
//! be logged as the exception. It is offered only if the concatenation is a string from its first `+` on (a string
//! literal among the first two operands), the format string is one literal with only `%s`, the literals have no `{}`
//! or backslash (escapes would change the placeholders), no value is locally known to be a throwable, a single
//! `String.format` argument is not known to be an array (it would be the varargs array), and the message has no
//! comments.

use crate::lint::logging::shared::{
    Api, Declared, Kind, constant_string, declaration, declares_type, imports, kind, log_call, names_type, type_import,
};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(call) = log_call(tree, &context.ancestors, context) else { return };
    if call.api == Api::Jul {
        return;
    }
    let mut ancestors = context.ancestors.clone();
    ancestors.push(tree);
    let index = usize::from(call.args.len() >= 2 && is_marker(&call.args[0], &ancestors, context));
    let Some(message) = call.args.get(index) else { return };
    let expression = strip_parens(message);
    let (description, fix) = if let Some(operands) = concatenation(expression) {
        ("concatenation", concatenation_fix(&operands, &ancestors, context))
    } else if let Some((format, values)) = format_call(expression, &ancestors) {
        ("String.format", format_fix(format, values, &ancestors, context))
    } else {
        return;
    };
    let text = format!("log message built by {description} instead of with placeholders");
    let diagnostic = Diagnostic::new(Rule::ConcatenatedLogMessage, message.start, text);
    let fix = fix.filter(|_| !context.tokens().has_comment(message.start..message.end));
    diagnostics.push(match fix {
        Some(replacement) => diagnostic.with_unsafe_fix(vec![Edit::new(message.start..message.end, replacement)]),
        None => diagnostic,
    });
}

/// Whether the argument is locally known to be a SLF4J or Log4j 2 `Marker`: a variable of that type or a call of
/// `MarkerFactory.getMarker` or `MarkerManager.getMarker`.
fn is_marker(argument: &Tree, ancestors: &[&Tree], context: &Context) -> bool {
    let Some(unit) = ancestors.first() else { return false };
    match &strip_parens(argument).kind {
        K::Ident(name) => match declaration(argument, ancestors, name, context) {
            Some(Declared::Var(var)) => var.ty.as_deref().is_some_and(|ty| {
                names_type(ty, "org.slf4j.Marker", unit) || names_type(ty, "org.apache.logging.log4j.Marker", unit)
            }),
            _ => false,
        },
        K::MethodInvocation { select, .. } => matches!(&select.kind, K::MemberSelect { expr, name }
            if name == "getMarker"
                && (names_type(expr, "org.slf4j.MarkerFactory", unit)
                    || names_type(expr, "org.apache.logging.log4j.MarkerManager", unit))),
        _ => false,
    }
}

/// The operands of a string concatenation with a string literal and a non-constant operand, from left to right.
fn concatenation(expression: &Tree) -> Option<Vec<&Tree>> {
    let mut operands = Vec::new();
    collect_operands(expression, &mut operands);
    let has_literal = operands.iter().any(|o| is_string_literal(o));
    let has_variable = operands.iter().any(|o| !is_constant(o));
    (operands.len() >= 2 && has_literal && has_variable).then_some(operands)
}

fn collect_operands<'t>(tree: &'t Tree, operands: &mut Vec<&'t Tree>) {
    match &tree.kind {
        K::Binary { op, lhs, rhs } if op == "+" => {
            collect_operands(lhs, operands);
            operands.push(rhs);
        }
        _ => operands.push(tree),
    }
}

fn is_string_literal(tree: &Tree) -> bool {
    matches!(strip_parens(tree).kind, K::Literal(LitKind::String))
}

/// Whether the operand is a literal or a name in upper case, such as `MAX` or `Limits.MAX_SIZE`.
fn is_constant(tree: &Tree) -> bool {
    let is_upper_case = |name: &str| {
        name.starts_with(|c: char| c.is_ascii_uppercase())
            && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
    };
    match &strip_parens(tree).kind {
        K::Literal(_) => true,
        K::Ident(name) | K::MemberSelect { name, .. } => is_upper_case(name),
        _ => false,
    }
}

/// The message with placeholders and the values for a concatenation, if the fix is possible.
fn concatenation_fix(operands: &[&Tree], ancestors: &[&Tree], context: &Context) -> Option<String> {
    if !operands.iter().take(2).any(|o| is_string_literal(o)) {
        return None;
    }
    let mut pattern = String::new();
    let mut values = Vec::new();
    for operand in operands {
        if is_string_literal(operand) {
            pattern.push_str(&literal_content(operand, context)?);
        } else {
            if kind(operand, ancestors, context) == Kind::Throwable {
                return None;
            }
            pattern.push_str("{}");
            values.push(context.source(operand));
        }
    }
    Some(arguments(&pattern, &values))
}

/// The call `String.format(format, values...)` or `format.formatted(values...)` with at least one value, as the
/// format and the values.
fn format_call<'t>(expression: &'t Tree, ancestors: &[&Tree]) -> Option<(&'t Tree, &'t [Tree])> {
    let unit = ancestors.first()?;
    let K::MethodInvocation { select, args, .. } = &expression.kind else { return None };
    let K::MemberSelect { expr, name } = &select.kind else { return None };
    match name.as_str() {
        "format" if is_java_lang_string(expr, unit) => {
            let (format, values) = args.split_first()?;
            (!values.is_empty()).then_some((format, values))
        }
        "formatted" if is_string_literal(expr) && !args.is_empty() => Some((expr, args)),
        _ => None,
    }
}

/// Whether the qualifier is `String` or `java.lang.String`, and `String` is not declared or imported in the file.
fn is_java_lang_string(tree: &Tree, unit: &Tree) -> bool {
    match &tree.kind {
        K::Ident(name) => {
            name == "String"
                && !declares_type(unit, name)
                && !imports(unit)
                    .iter()
                    .filter_map(type_import)
                    .any(|(import, on_demand)| !on_demand && import.rsplit('.').next() == Some(name))
        }
        K::MemberSelect { .. } => names_type(tree, "java.lang.String", unit),
        _ => false,
    }
}

/// The message with placeholders and the values for a format call, if the fix is possible.
fn format_fix(format: &Tree, values: &[Tree], ancestors: &[&Tree], context: &Context) -> Option<String> {
    if !is_string_literal(format) {
        return None;
    }
    let content = literal_content(format, context)?;
    let specifiers = content.matches("%s").count();
    if specifiers != values.len() || content.matches('%').count() != specifiers {
        return None;
    }
    let kinds: Vec<Kind> = values.iter().map(|v| kind(v, ancestors, context)).collect();
    if kinds.contains(&Kind::Throwable) || kinds == [Kind::Other { array: true }] {
        return None;
    }
    let values: Vec<&str> = values.iter().map(|v| context.source(v)).collect();
    Some(arguments(&content.replace("%s", "{}"), &values))
}

/// The source between the quotes of a string literal, if its value has no `{}` and no backslash.
fn literal_content(literal: &Tree, context: &Context) -> Option<String> {
    let value = constant_string(literal, context.text)?;
    if value.contains("{}") || value.contains('\\') {
        return None;
    }
    let source = context.source(strip_parens(literal));
    Some(source.strip_prefix('"')?.strip_suffix('"')?.to_string())
}

fn arguments(pattern: &str, values: &[&str]) -> String {
    let mut arguments = format!("\"{pattern}\"");
    for value in values {
        arguments.push_str(", ");
        arguments.push_str(value);
    }
    arguments
}
