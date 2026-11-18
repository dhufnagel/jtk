//! A006: more than one top-level type in a file (Checkstyle OneTopLevelClass).
//!
//! As in Checkstyle, the public type (or else the first one) may stay; every other top-level type is reported.

use super::{has_keyword, top_level_types};
use crate::ast::Tree;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(unit: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if top_level_types(unit).nth(1).is_none() {
        return;
    }
    let types: Vec<_> = top_level_types(unit)
        .map(|(tree, class)| (tree, &class.name, has_keyword(&class.mods, class.name_pos, "public", context)))
        .collect();
    let Some(&(_, _, first_is_public)) = types.first() else { return };
    let any_public = types.iter().any(|(_, _, is_public)| *is_public);
    let reported = types.iter().enumerate().filter(
        |(i, (_, _, is_public))| {
            if *i == 0 { !first_is_public && any_public } else { !is_public }
        },
    );
    for (_, (tree, name, _)) in reported {
        let message = format!("Top-level class {name} has to reside in its own source file");
        diagnostics.push(Diagnostic::new(Rule::OneTopLevelClass, tree.start, message));
    }
}
