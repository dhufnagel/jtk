//! S129: a conversion of a value to the type it has already (Picnic IdentityConversion, NetBeans and IntelliJ boxing
//! of an already boxed value): `String.valueOf(s)` and `s.toString()` of a `String s`, `Integer.valueOf(i)` of an
//! `Integer i` (and the other wrappers).
//!
//! Without type information, only a variable, parameter or field declared in the file with the type (of
//! `java.lang`) is reported. The fix writes the variable alone. It is unsafe: where the variable is null, the
//! conversion returns `"null"` (`String.valueOf`) or throws, and `Integer.valueOf` may return another instance, which
//! `==` tells apart.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

use super::shared_batch3::{class_name, declared_type, refers_to};

const TYPES: &[&str] = &["String", "Integer", "Long", "Short", "Byte", "Character", "Boolean", "Double", "Float"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { select, type_args, args } = &tree.kind else { return };
    let K::MemberSelect { expr: receiver, name: method } = &select.kind else { return };
    if !type_args.is_empty() {
        return;
    }
    let (variable, converted_to, call) = match (method.as_str(), args.as_slice()) {
        ("toString", []) => (&**receiver, "String".to_string(), "toString()"),
        ("valueOf", [argument]) if !is_variable(receiver, tree, context) => {
            let Some(class) = class_name(receiver, context) else { return };
            (argument, class, "valueOf()")
        }
        _ => return,
    };
    let K::Ident(name) = &variable.kind else { return };
    let Some(ty) = declared_type(tree, &context.ancestors, name, context).and_then(|t| class_name(t, context)) else {
        return;
    };
    let ancestors = &context.ancestors;
    let same = TYPES.iter().find(|t| {
        let qualified = format!("java.lang.{t}");
        refers_to(&ty, &qualified, ancestors, context) && refers_to(&converted_to, &qualified, ancestors, context)
    });
    let Some(same) = same else { return };
    let message = format!("Unnecessary {call} of a value that is a {same} already");
    let diagnostic = Diagnostic::new(Rule::IdentityConversion, tree.start, message);
    diagnostics.push(if context.tokens().has_comment(tree.start..tree.end) {
        diagnostic
    } else {
        diagnostic.with_unsafe_fix(vec![Edit::new(tree.start..tree.end, name.clone())])
    });
}

/// Whether the receiver of `valueOf` may be a variable rather than a class.
fn is_variable(receiver: &Tree, tree: &Tree, context: &Context) -> bool {
    matches!(&receiver.kind, K::Ident(name) if declared_type(tree, &context.ancestors, name, context).is_some())
}
