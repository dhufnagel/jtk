//! A007: a file without a package declaration, or whose package does not match its directory (Checkstyle
//! PackageDeclaration with `matchDirectoryStructure`).
//!
//! As in Checkstyle, the directory matches if its path ends with the package's path (as text), module declarations
//! and compact source files (with top-level methods) need no package, and the missing package is reported at the
//! first declaration of the file.

use std::path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR, Path};

use tree_sitter::Node;

use super::package_name;
use crate::lint::syntax::{is_comment, named_children};
use crate::lint::{Diagnostic, Rule};

pub fn check(program: &Node, text: &str, path: &Path, diagnostics: &mut Vec<Diagnostic>) {
    let children: Vec<Node> = named_children(program).into_iter().filter(|c| !is_comment(c)).collect();
    let Some(first) = children.first() else { return };
    let Some(declaration) = children.iter().find(|c| c.kind() == "package_declaration") else {
        let is_compact = children.iter().any(|c| c.kind() == "method_declaration" || is_statement(c));
        let is_module = children.iter().any(|c| c.kind() == "module_declaration");
        if !is_compact && !is_module {
            diagnostics.push(Diagnostic::new(
                Rule::PackageDirectory,
                first.start_byte(),
                "Missing package declaration",
            ));
        }
        return;
    };
    let package = package_name(program, text).replace('.', MAIN_SEPARATOR_STR);
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let path = path.to_string_lossy();
    let directory = path.rfind(MAIN_SEPARATOR).map_or("", |i| &path[..i]);
    if !directory.ends_with(&package) {
        let message = format!("Package name '{}' does not match the directory", package.replace(MAIN_SEPARATOR, "."));
        let keyword = declaration.children(&mut declaration.walk()).find(|c| c.kind() == "package");
        let offset = keyword.map_or(declaration.start_byte(), |k| k.start_byte());
        diagnostics.push(Diagnostic::new(Rule::PackageDirectory, offset, message));
    }
}

/// Whether a top-level node is a statement or field of a compact source file rather than a declaration.
fn is_statement(node: &Node) -> bool {
    !matches!(
        node.kind(),
        "package_declaration"
            | "import_declaration"
            | "module_declaration"
            | "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "annotation_type_declaration"
            | "record_declaration"
    ) && node.is_named()
}
