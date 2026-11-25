//! M043: `Paths.get(...)` instead of `Path.of(...)` (Modernizer, Picnic FileRules PathOf and PathOfVarargs): since
//! Java 11, `Path.of` is the factory, and `Paths.get` only calls it.
//!
//! Safe fix: `Path.of(...)`, which does the same. `Paths` must be `java.nio.file.Paths` through the file's imports
//! (or written out); there is no fix if `Path` names another type in the file. As the calls share the imports, the
//! fix of each call changes all calls of the file at once: it imports `java.nio.file.Path` unless the file already
//! does (alone, or on demand without other on-demand imports that could also have a `Path`), and drops the import of
//! `Paths` once nothing else (a method reference, a comment such as a Javadoc link) uses it. Calls with comments in
//! `Paths.get` keep it.

use crate::lint::logging::shared::{declares_type, imports, imports_type, qualified_name, type_import};
use crate::lint::tree::{Context, for_each_descendant, span_removal_range};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

const PATHS: &str = "java.nio.file.Paths";
const PATH: &str = "java.nio.file.Path";

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { select, type_args, .. } = &tree.kind else { return };
    let K::MemberSelect { expr: qualifier, name } = &select.kind else { return };
    let Some(unit) = context.ancestors.first() else { return };
    let simple = matches!(&qualifier.kind, K::Ident(name) if name == "Paths") && imports_type(unit, "Paths", PATHS);
    let qualified = matches!(qualifier.kind, K::MemberSelect { .. }) && qualified_name(qualifier) == PATHS;
    if name != "get" || !type_args.is_empty() || !(simple || qualified) {
        return;
    }
    let diagnostic = Diagnostic::new(Rule::PathOf, tree.start, "Paths.get instead of Path.of");
    let tokens = context.tokens();
    let edits = if tokens.has_comment(select.start..select.end) {
        None
    } else if qualified {
        Some(vec![Edit::new(select.start..select.end, format!("{PATH}.of"))])
    } else {
        simple_name_edits(unit, context)
    };
    diagnostics.push(match edits {
        Some(edits) => diagnostic.with_fix(edits),
        None => diagnostic,
    });
}

/// The edits that change all calls `Paths.get` of the file, with the imports.
fn simple_name_edits(unit: &Tree, context: &Context) -> Option<Vec<Edit>> {
    let path_imported = imports_type(unit, "Path", PATH);
    let other_path = imports(unit)
        .iter()
        .filter_map(type_import)
        .any(|(name, on_demand)| !on_demand && name.rsplit('.').next() == Some("Path") && name != PATH);
    if !path_imported && (other_path || declares_type(unit, "Path")) {
        return None;
    }
    let mut edits = Vec::new();
    let mut other_uses = context.tokens().comments().any(|c| context.text[c].contains("Paths"));
    for_each_descendant(unit, &mut |tree, parent| match &tree.kind {
        K::Import { .. } => false,
        K::Ident(name) if name == "Paths" => {
            other_uses |= !matches!(&parent.kind, K::MemberSelect { name, .. } if name == "get")
                || !edits.iter().any(|e: &Edit| e.range.start == tree.start);
            true
        }
        K::MethodInvocation { select, type_args, .. } if type_args.is_empty() => {
            if let K::MemberSelect { expr, name } = &select.kind
                && name == "get"
                && matches!(&expr.kind, K::Ident(q) if q == "Paths")
                && !context.tokens().has_comment(select.start..select.end)
            {
                edits.push(Edit::new(select.start..select.end, "Path.of"));
            }
            true
        }
        _ => true,
    });
    let paths_import =
        imports(unit).iter().find(|i| type_import(i).is_some_and(|(name, on_demand)| !on_demand && name == PATHS));
    // A `Path` imported on demand may clash with a `Path` of another package imported on demand.
    let on_demand_imports = imports(unit).iter().filter_map(type_import).filter(|(_, on_demand)| *on_demand).count();
    let needs_import = !path_imported
        || on_demand_imports > 1
            && !imports(unit).iter().filter_map(type_import).any(|(name, on_demand)| !on_demand && name == PATH);
    match paths_import {
        Some(import) if !other_uses => {
            if needs_import {
                let K::Import { name, .. } = &import.kind else { return None };
                let segment = context.tokens().previous(name.end)?;
                edits.push(Edit::new(segment, "Path"));
            } else {
                edits.push(Edit::new(span_removal_range(import.start..import.end, context.text), ""));
            }
        }
        _ if needs_import => {
            let first = paths_import.or(imports(unit).first())?;
            edits.push(Edit::new(first.start..first.start, format!("import {PATH};\n")));
        }
        _ => {}
    }
    Some(edits)
}
