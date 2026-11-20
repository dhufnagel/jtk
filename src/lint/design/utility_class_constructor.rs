//! A001: a utility class (only static members, at least one of them not private) with a public or default
//! constructor (Checkstyle HideUtilityClassConstructor, PMD InstantiableUtilityClass).
//!
//! As in Checkstyle, abstract and `static` classes and classes that extend another class are not utility classes,
//! and a protected constructor hides the constructor too.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Modifier, Tree};

pub fn check(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    if class.kind != ClassKind::Class
        || class.extends.is_some()
        || class.mods.has(Modifier::Abstract)
        || class.mods.has(Modifier::Static)
    {
        return;
    }
    let mut has_visible_static_member = false;
    let mut has_constructor = false;
    let mut has_visible_constructor = false;
    for member in &class.members {
        let (mods, is_constructor) = match &member.kind {
            K::Method(method) => (&method.mods, method.ret.is_none()),
            K::Vars(vars) => (&vars[0].mods, false),
            _ => continue,
        };
        if is_constructor {
            has_constructor = true;
            if !mods.has(Modifier::Private) && !mods.has(Modifier::Protected) {
                has_visible_constructor = true;
            }
        } else if !mods.has(Modifier::Static) {
            // An instance member: not a utility class.
            return;
        } else if !mods.has(Modifier::Private) {
            has_visible_static_member = true;
        }
    }
    if has_visible_static_member && (!has_constructor || has_visible_constructor) {
        let message = "Utility classes should not have a public or default constructor";
        diagnostics.push(Diagnostic::new(Rule::UtilityClassConstructor, tree.start, message));
    }
}
