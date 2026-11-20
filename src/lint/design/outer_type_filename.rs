//! A017: the first top-level type is not named like the file, and no other top-level type is public (Checkstyle
//! OuterTypeFilename).
//!
//! As in Checkstyle, a file with a public top-level type after the first one is left to the compiler, and compact
//! source files (with top-level methods) are not checked.

use super::top_level_types;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Modifier, Tree};

pub fn check(unit: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(path) = context.path else { return };
    let K::CompilationUnit { types, .. } = &unit.kind else { return };
    if types.iter().any(|t| matches!(t.kind, K::Method(_))) {
        return;
    }
    let mut types = top_level_types(unit);
    let Some((first, class)) = types.next() else { return };
    let file_name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let file_stem = file_name.rfind('.').map_or(&*file_name, |i| &file_name[..i]);
    if class.name != file_stem && !types.any(|(_, other)| other.mods.has(Modifier::Public)) {
        let message = "The name of the outer type and the file do not match";
        diagnostics.push(Diagnostic::new(Rule::OuterTypeFilename, first.start, message));
    }
}
