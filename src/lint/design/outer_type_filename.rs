//! A017: the first top-level type is not named like the file, and no other top-level type is public (Checkstyle
//! OuterTypeFilename).
//!
//! As in Checkstyle, a file with a public top-level type after the first one is left to the compiler, and compact
//! source files (with top-level methods) are not checked.

use std::path::Path;

use tree_sitter::Node;

use super::{has_keyword, is_type_declaration};
use crate::lint::syntax::{named_children, source};
use crate::lint::{Diagnostic, Rule};

pub fn check(program: &Node, text: &str, path: &Path, diagnostics: &mut Vec<Diagnostic>) {
    let children = named_children(program);
    if children.iter().any(|c| c.kind() == "method_declaration") {
        return;
    }
    let types: Vec<&Node> = children.iter().filter(|c| is_type_declaration(c)).collect();
    let Some(first) = types.first() else { return };
    let file_name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
    let file_stem = file_name.rfind('.').map_or(&*file_name, |i| &file_name[..i]);
    let name = first.child_by_field_name("name").map_or("", |n| source(&n, text));
    let has_public = types.iter().skip(1).any(|t| has_keyword(t, text, "public"));
    if name != file_stem && !has_public {
        let message = "The name of the outer type and the file do not match";
        diagnostics.push(Diagnostic::new(Rule::OuterTypeFilename, first.start_byte(), message));
    }
}
