//! A007: a file without a package declaration, or whose package does not match its directory (Checkstyle
//! PackageDeclaration with `matchDirectoryStructure`).
//!
//! As in Checkstyle, the directory matches if its path ends with the package's path (as text), module declarations
//! and compact source files (with top-level methods) need no package, and the missing package is reported at the
//! first declaration of the file.

use std::path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR};

use super::qualified_name;
use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(unit: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(path) = context.path else { return };
    let K::CompilationUnit { package, imports, types, module } = &unit.kind else { return };
    let Some((_, name)) = package else {
        let is_compact = types.iter().any(|t| !matches!(t.kind, K::Class(_)));
        let first = [imports.first(), types.first(), module.as_deref()].into_iter().flatten().map(|t| t.start).min();
        if let Some(first) = first
            && !is_compact
            && module.is_none()
        {
            diagnostics.push(Diagnostic::new(Rule::PackageDirectory, first, "Missing package declaration"));
        }
        return;
    };
    let package = qualified_name(name).replace('.', MAIN_SEPARATOR_STR);
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let path = path.to_string_lossy();
    let directory = path.rfind(MAIN_SEPARATOR).map_or("", |i| &path[..i]);
    if !directory.ends_with(&package) {
        let message = format!("Package name '{}' does not match the directory", package.replace(MAIN_SEPARATOR, "."));
        let keyword = context.tokens().previous(name.start).map_or(name.start, |k| k.start);
        diagnostics.push(Diagnostic::new(Rule::PackageDirectory, keyword, message));
    }
}
