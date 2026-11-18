//! I002: an import from the `sun` packages or others set in `illegal-pkgs` (Checkstyle IllegalImport, PMD
//! DontImportSun): they are JDK internals that can change or disappear in any release.

use super::imported_name;
use crate::ast::Tree;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(import: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let name = imported_name(import, context.text);
    let packages = &context.options.illegal_import.illegal_pkgs;
    let is_illegal = packages.iter().any(|p| name.strip_prefix(p).is_some_and(|rest| rest.starts_with('.')));
    if is_illegal {
        diagnostics.push(Diagnostic::new(Rule::IllegalImport, import.start, format!("Illegal import - {name}")));
    }
}
