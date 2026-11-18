//! A001: a utility class (only static members, at least one of them not private) with a public or default
//! constructor (Checkstyle HideUtilityClassConstructor, PMD InstantiableUtilityClass).
//!
//! As in Checkstyle, abstract and `static` classes and classes that extend another class are not utility classes,
//! and a protected constructor hides the constructor too.

use super::has_keyword;
use crate::ast::{ClassKind, K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    let class_has = |keyword| has_keyword(&class.mods, class.name_pos, keyword, context);
    if class.kind != ClassKind::Class || class.extends.is_some() || class_has("abstract") || class_has("static") {
        return;
    }
    let mut has_visible_static_member = false;
    let mut has_constructor = false;
    let mut has_visible_constructor = false;
    for member in &class.members {
        let (mods, name_pos, is_constructor) = match &member.kind {
            K::Method(method) => (&method.mods, method.name_pos, method.ret.is_none()),
            K::Vars(vars) => (&vars[0].mods, vars[0].name_pos, false),
            _ => continue,
        };
        let has = |keyword| has_keyword(mods, name_pos, keyword, context);
        if is_constructor {
            has_constructor = true;
            if !has("private") && !has("protected") {
                has_visible_constructor = true;
            }
        } else if !has("static") {
            // An instance member: not a utility class.
            return;
        } else if !has("private") {
            has_visible_static_member = true;
        }
    }
    if has_visible_static_member && (!has_constructor || has_visible_constructor) {
        let message = "Utility classes should not have a public or default constructor";
        diagnostics.push(Diagnostic::new(Rule::UtilityClassConstructor, tree.start, message));
    }
}
