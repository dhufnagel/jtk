//! B142: `"."` as the regular expression of `split`, `replaceAll` or `replaceFirst`, which matches any character, not
//! a dot (Error Prone BareDotMetacharacter, IntelliJ ReplaceAllDot, NetBeans Single Char Regex).
//!
//! Like Error Prone, the receiver is taken for a `String` unless it is known not to be one (a variable declared with
//! another type, a class name). `split(".")` is always wrong (it returns an empty array); `replaceAll(".", r)` may be
//! meant (masking each character), so the fix is unsafe: `"\\."` to match a dot.

use super::shared_misc::is_string;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { select, args, .. } = &tree.kind else { return };
    let K::MemberSelect { expr: receiver, name } = &select.kind else { return };
    let is_regex_method = matches!((name.as_str(), args.len()), ("split", 1 | 2) | ("replaceAll" | "replaceFirst", 2));
    let Some(regex) = args.first() else { return };
    if !is_regex_method || context.source(regex) != "\".\"" || is_string(receiver, tree, context) == Some(false) {
        return;
    }
    let message = format!("\".\" as the regular expression of {name}() matches any character");
    let fix = vec![Edit::new(regex.start..regex.end, "\"\\\\.\"")];
    diagnostics.push(Diagnostic::new(Rule::RegexDot, regex.start, message).with_unsafe_fix(fix));
}
