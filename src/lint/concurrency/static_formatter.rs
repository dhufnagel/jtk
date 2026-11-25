//! K047: a static field holding a `java.text` formatter (`SimpleDateFormat`, `DateFormat`, `NumberFormat`,
//! `DecimalFormat`, `MessageFormat`, `ChoiceFormat`): these classes are not thread-safe, and a static instance is
//! easily shared between threads (Error Prone DateFormatConstant, PMD UnsynchronizedStaticFormatter, IntelliJ
//! AccessToNonThreadSafeStaticFieldFromInstance).
//!
//! No fix: `DateTimeFormatter` has a different pattern syntax and API.
//!
//! Like Error Prone, the declaration is reported (final or not), not each access from unsynchronized code as in PMD:
//! a formatter used only under a lock is reported too. The type must resolve to `java.text` through the imports;
//! fields of interfaces are implicitly static.

use super::shared::names_type;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Modifier, Tree};

const FORMATTERS: &[&str] = &[
    "java.text.SimpleDateFormat",
    "java.text.DateFormat",
    "java.text.NumberFormat",
    "java.text.DecimalFormat",
    "java.text.MessageFormat",
    "java.text.ChoiceFormat",
];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Vars(vars) = &tree.kind else { return };
    let Some(K::Class(class)) = context.parent().map(|p| &p.kind) else { return };
    let in_interface = matches!(class.kind, ClassKind::Interface | ClassKind::Annotation);
    if !class.members.iter().any(|m| std::ptr::eq(m, tree)) {
        return;
    }
    for var in vars {
        let is_static = var.mods.has(Modifier::Static) || in_interface;
        // `SimpleDateFormat f[]` is an array.
        let Some(ty) = var.ty.as_deref().filter(|ty| is_static && ty.end <= var.name_pos) else { continue };
        if let Some(class) = FORMATTERS.iter().find(|c| names_type(ty, c, context)) {
            let simple = class.rsplit('.').next().unwrap_or(class);
            let message = format!("{simple} is not thread-safe but held in a static field");
            diagnostics.push(Diagnostic::new(Rule::StaticFormatter, var.name_pos, message));
        }
    }
}
