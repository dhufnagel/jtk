//! Rules of group I: imports, beyond the unused and unsorted ones the formatter fixes.

mod fully_qualified_type;
mod illegal_import;
mod star_import;
mod static_import;

use super::tree::{Context, TreeCheck};
use super::{Check, ConfiguredCheck, Rule};
use crate::ast::{K, Tag, Tree};

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], TreeCheck)] = &[
    (Rule::StarImport, &[Tag::Import], star_import::check),
    (Rule::StaticImport, &[Tag::Import], static_import::check),
    (Rule::IllegalImport, &[Tag::Import], illegal_import::check),
    (Rule::FullyQualifiedType, &[Tag::CompilationUnit], fully_qualified_type::check),
];

pub(super) const CHECKS: &[(Rule, &[&str], Check)] = &[];

/// The checks of rules with parameters.
pub(super) const CONFIGURED_CHECKS: &[(Rule, &[&str], ConfiguredCheck)] = &[];

/// The name an import imports, and whether the import is on demand: the name of `import a.b.*;` is `a.b`.
fn import_name(import: &Tree) -> Option<(&Tree, bool)> {
    let K::Import { name, .. } = &import.kind else { return None };
    match &name.kind {
        K::MemberSelect { expr, name } if name == "*" => Some((expr, true)),
        _ => Some((name, false)),
    }
}

fn is_on_demand(import: &Tree) -> bool {
    import_name(import).is_some_and(|(_, on_demand)| on_demand)
}

/// The imported name as Checkstyle spells it: without whitespace, ending in `.*` for an on-demand import.
fn imported_name(import: &Tree, text: &str) -> String {
    let Some((name, on_demand)) = import_name(import) else { return String::new() };
    let mut imported: String = text[name.start..name.end].split_whitespace().collect();
    if on_demand {
        imported.push_str(".*");
    }
    imported
}

/// The offset of the last `.` of the imported name, where Checkstyle reports violations about the name.
fn last_dot(import: &Tree, context: &Context) -> usize {
    match &import.kind {
        K::Import { name, .. } => match &name.kind {
            K::MemberSelect { expr, .. } => context.tokens().next(expr.end).map_or(import.start, |dot| dot.start),
            _ => import.start,
        },
        _ => import.start,
    }
}
