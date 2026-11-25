//! B140: `new StringBuilder('a')` or `new StringBuffer('a')`: the `char` is widened to the `int` capacity, so the
//! builder starts empty instead of with the character (IntelliJ NewStringBufferWithCharArgument, Error Prone
//! StringBuilderInitWithChar, NetBeans, PMD StringBufferInstantiationWithChar).
//!
//! Only character literals are reported, with `StringBuilder` and `StringBuffer` resolved to `java.lang`. The fix
//! turns the literal into a string literal, `new StringBuilder("a")`. It is safe in the catalogue's sense: it changes
//! the behaviour, but an empty builder with a capacity given as a character is never what is meant.

use super::shared_misc::names_type;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::NewClass { ident, args, body: None, .. } = &tree.kind else { return };
    let [argument] = args.as_slice() else { return };
    let K::Literal(LitKind::Char) = argument.kind else { return };
    let Some(class) = ["StringBuilder", "StringBuffer"]
        .into_iter()
        .find(|class| names_type(ident, &format!("java.lang.{class}"), context))
    else {
        return;
    };
    let message = format!("new {class}(char) sets the capacity, not the content");
    let diagnostic = Diagnostic::new(Rule::StringBuilderChar, tree.start, message);
    diagnostics.push(match string_literal(context.source(argument)) {
        Some(literal) => diagnostic.with_fix(vec![Edit::new(argument.start..argument.end, literal)]),
        None => diagnostic,
    });
}

/// The string literal with the character of the character literal: the same escape, except for quotes. `None` for
/// a Unicode escape, which may stand for a quote.
fn string_literal(char_literal: &str) -> Option<String> {
    let content = char_literal.strip_prefix('\'')?.strip_suffix('\'')?;
    Some(match content {
        _ if content.contains("\\u") => return None,
        "\"" => "\"\\\"\"".to_string(),
        "\\'" => "\"'\"".to_string(),
        _ => format!("\"{content}\""),
    })
}
