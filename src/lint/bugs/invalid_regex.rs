//! B143: a string literal passed as a regular expression that is not valid Java regular expression syntax, so that
//! the call throws a `PatternSyntaxException` (Error Prone InvalidPatternSyntax, NetBeans Regexp).
//!
//! Checked are `Pattern.compile(regex)`, `Pattern.compile(regex, flags)` with flags that do not change the syntax
//! (not `COMMENTS` or `LITERAL`), `Pattern.matches(regex, input)` with `Pattern` resolved to `java.util.regex.Pattern`
//! through the imports, and the `String` methods `split`, `replaceAll` and `replaceFirst` unless the receiver is known
//! not to be a `String` (a variable declared with another type, a class name); `matches` only on a receiver known to
//! be a `String`, as other types have `matches` methods taking globs and such. Only a single string literal is
//! checked, not text blocks or constants, and only errors that are certain without compiling the pattern (see
//! `regex_error`). No fix.

use super::shared_misc::{is_string, names_type, regex_error, string_value};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

/// The flags of `Pattern.compile` that leave the syntax as it is.
const SYNTAX_PRESERVING_FLAGS: &[&str] =
    &["CASE_INSENSITIVE", "MULTILINE", "DOTALL", "UNICODE_CASE", "CANON_EQ", "UNIX_LINES", "UNICODE_CHARACTER_CLASS"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { select, args, .. } = &tree.kind else { return };
    let K::MemberSelect { expr: receiver, name } = &select.kind else { return };
    let Some(regex) = args.first() else { return };
    let is_pattern = || names_type(receiver, "java.util.regex.Pattern", context);
    let checked = match (name.as_str(), args.len()) {
        ("compile", 1) | ("matches", 2) => is_pattern(),
        ("compile", 2) => is_pattern() && preserves_syntax(&args[1], context.text),
        ("split", 1 | 2) | ("replaceAll" | "replaceFirst", 2) => is_string(receiver, tree, context) != Some(false),
        ("matches", 1) => is_string(receiver, tree, context) == Some(true),
        _ => false,
    };
    if !checked {
        return;
    }
    let Some(error) = string_value(regex, context.text).and_then(|value| regex_error(&value)) else { return };
    diagnostics.push(Diagnostic::new(Rule::InvalidRegex, regex.start, format!("Invalid regular expression: {error}")));
}

/// Whether the flags are `0` or a combination of flags that do not change the syntax.
fn preserves_syntax(flags: &Tree, text: &str) -> bool {
    match &flags.kind {
        K::Literal(LitKind::Int) => &text[flags.start..flags.end] == "0",
        K::Binary { op, lhs, rhs } if op == "|" => preserves_syntax(lhs, text) && preserves_syntax(rhs, text),
        K::Ident(name) | K::MemberSelect { name, .. } => SYNTAX_PRESERVING_FLAGS.contains(&name.as_str()),
        K::Parens(inner) => preserves_syntax(inner, text),
        _ => false,
    }
}
