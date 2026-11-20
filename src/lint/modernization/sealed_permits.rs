//! M014 and M016: the `permits` clause of sealed types, Java 17.
//!
//! - M014: a `sealed` class or interface without `permits` (Checkstyle SealedShouldHavePermitsList). No fix.
//! - M016: a `permits` clause that only names types declared in the same file (Checkstyle UnnecessaryPermitsClause):
//!   the compiler infers it. As in Checkstyle, types are compared by simple name with all classes, interfaces, enums
//!   and records of the file. The fix removes the clause.

use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{ClassKind, K, Modifier, Tree};

pub fn missing_permits(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    if matches!(class.kind, ClassKind::Class | ClassKind::Interface)
        && class.permits.is_empty()
        && class.mods.has(Modifier::Sealed)
    {
        let message = "Sealed classes or interfaces should explicitly declare permitted subclasses";
        diagnostics.push(Diagnostic::new(Rule::SealedPermits, tree.start, message));
    }
}

pub fn unnecessary_permits(unit: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    // Most files have no `permits` clause; finding that in the text is much cheaper than walking the tree.
    if !context.text.contains("permits") {
        return;
    }
    let mut names = Vec::new();
    let mut clauses = Vec::new();
    for_each_descendant(unit, &mut |tree, _| {
        if let K::Class(class) = &tree.kind
            && class.kind != ClassKind::Annotation
        {
            names.push(class.name.as_str());
            if !class.permits.is_empty() {
                clauses.push(&class.permits);
            }
        }
        true
    });
    for permits in clauses {
        let mut permitted = Vec::new();
        for permit in permits {
            simple_names(permit, &mut permitted);
        }
        let (Some(first), Some(last)) = (permits.first(), permits.last()) else { continue };
        let Some(keyword) = context.tokens().previous(first.start) else { continue };
        if permitted.iter().all(|p| names.contains(p)) {
            // Only the blanks on the same line: a line comment may end the line before.
            let before = context.text[..keyword.start].trim_end_matches([' ', '\t']).len();
            let fix = Edit::new(before..last.end, "");
            let message = "Unnecessary 'permits' clause for classes in the same compilation unit";
            diagnostics.push(Diagnostic::new(Rule::UnnecessaryPermits, keyword.start, message).with_fix(vec![fix]));
        }
    }
}

/// The simple names of the classes a type names: `C` for `a.b.C`, and those of type arguments.
fn simple_names<'t>(ty: &'t Tree, names: &mut Vec<&'t str>) {
    match &ty.kind {
        K::Ident(name) | K::MemberSelect { name, .. } => names.push(name),
        K::Annotated { base, .. } => simple_names(base, names),
        _ => ty.for_each_child(&mut |child| simple_names(child, names)),
    }
}
