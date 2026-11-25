//! B148: `LocalDate.now()` and the other `now()` factories of `java.time` without a zone or clock (Error Prone
//! JavaTimeDefaultTimeZone, Baseline JavaTimeSystemDefaultTimeZone, Picnic TimeZoneUsage, Sonar S8688): they use the
//! system's default time zone, which depends on the machine and is easily overlooked.
//!
//! Safe fix: `now(ZoneId.systemDefault())`, which is what `now()` does (`now(Clock.systemDefaultZone())`), but makes
//! the default zone visible (Error Prone's fix). `ZoneId` is written by its simple name if it is imported (alone, or
//! on demand with `java.time.*` while the file already uses the simple name); otherwise the fix imports it before
//! the first import that sorts after it, unless the simple name is imported from elsewhere, declared in the file or
//! used for something else; failing that (or without any import), it writes `java.time.ZoneId`. Only one fix per
//! round adds the import: it rewrites a neighbouring import, so the fixes of the other calls overlap it and wait for
//! the next round. No fix with comments inside the parentheses.
//!
//! Limits: the class must be named by its qualified name or a simple name imported from `java.time` (or
//! `java.time.chrono`) and not declared in the file (nor, if imported on demand, in a file of the same name next to
//! it); statically imported `now()` is not reported. `Instant.now()` and `Clock` are fine (no zone).

use crate::lint::logging::shared::{declares_type, imports, names_type, type_import};
use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

/// The classes whose `now()` uses the default time zone.
const CLASSES: &[&str] = &[
    "java.time.LocalDate",
    "java.time.LocalDateTime",
    "java.time.LocalTime",
    "java.time.MonthDay",
    "java.time.OffsetDateTime",
    "java.time.OffsetTime",
    "java.time.Year",
    "java.time.YearMonth",
    "java.time.ZonedDateTime",
    "java.time.chrono.HijrahDate",
    "java.time.chrono.JapaneseDate",
    "java.time.chrono.MinguoDate",
    "java.time.chrono.ThaiBuddhistDate",
];

const ZONE_ID: &str = "java.time.ZoneId";

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { select, type_args, args } = &tree.kind else { return };
    let K::MemberSelect { expr: class, name } = &select.kind else { return };
    let Some(unit) = context.ancestors.first() else { return };
    let declared = matches!(&class.kind, K::Ident(simple) if declares_type(unit, simple));
    if name != "now" || !args.is_empty() || !type_args.is_empty() || declared {
        return;
    }
    let Some(class_name) = CLASSES.iter().find(|c| names_type(class, c, unit)) else { return };
    if matches!(&class.kind, K::Ident(simple) if may_be_in_package(simple, unit, context)) {
        return;
    }
    let simple = class_name.rsplit('.').next().unwrap_or(class_name);
    let diagnostic =
        Diagnostic::new(Rule::DefaultTimeZone, tree.start, format!("{simple}.now() uses the default time zone"));
    let arguments = select.end..tree.end;
    if context.tokens().has_comment(arguments.clone()) {
        diagnostics.push(diagnostic);
        return;
    }
    let mut edits = Vec::new();
    let zone_id = match zone_id_reference(unit, context) {
        Reference::Simple => "ZoneId",
        Reference::Import(edit) => {
            edits.push(edit);
            "ZoneId"
        }
        Reference::Qualified => ZONE_ID,
    };
    edits.push(Edit::new(arguments, format!("({zone_id}.systemDefault())")));
    diagnostics.push(diagnostic.with_fix(edits));
}

/// Whether the simple name, imported only on demand, may name a class of the file's own package, which takes
/// precedence: a file of that name is next to the checked one.
fn may_be_in_package(simple: &str, unit: &Tree, context: &Context) -> bool {
    let single = imports(unit)
        .iter()
        .filter_map(type_import)
        .any(|(name, on_demand)| !on_demand && name.rsplit('.').next() == Some(simple));
    let sibling = context.path.and_then(|p| p.parent()).map(|dir| dir.join(format!("{simple}.java")));
    !single && sibling.is_some_and(|file| file.exists())
}

/// How the fix refers to `java.time.ZoneId`.
enum Reference {
    /// By the simple name, which already refers to it.
    Simple,
    /// By the simple name, with this edit that imports it.
    Import(Edit),
    /// By the qualified name.
    Qualified,
}

fn zone_id_reference(unit: &Tree, context: &Context) -> Reference {
    let type_imports: Vec<(String, bool)> = imports(unit).iter().filter_map(type_import).collect();
    let single = type_imports.iter().find(|(name, on_demand)| !on_demand && name.rsplit('.').next() == Some("ZoneId"));
    let declared = declares_type(unit, "ZoneId");
    match single {
        Some((name, _)) if name == ZONE_ID && !declared => return Reference::Simple,
        Some(_) => return Reference::Qualified,
        None if declared => return Reference::Qualified,
        None => {}
    }
    // Without a single-type import, a simple name already used in the file may refer to a type of the same package
    // or an inherited member type.
    let used = uses_simple_name(unit);
    let on_demand = type_imports.iter().any(|(name, on_demand)| *on_demand && name == "java.time");
    match import_edit(unit, context) {
        _ if used && on_demand => Reference::Simple,
        Some(edit) if !used => Reference::Import(edit),
        _ => Reference::Qualified,
    }
}

/// Whether the simple name `ZoneId` appears outside the imports.
fn uses_simple_name(unit: &Tree) -> bool {
    let mut used = false;
    for_each_descendant(unit, &mut |tree, _| {
        used |= matches!(&tree.kind, K::Ident(name) if name == "ZoneId");
        !used && !matches!(tree.kind, K::Import { .. })
    });
    used
}

/// The edit that imports `java.time.ZoneId` in sorted position: it rewrites the first type import that sorts after
/// it, or the last import, so that the edits of two calls overlap. `None` without imports.
fn import_edit(unit: &Tree, context: &Context) -> Option<Edit> {
    let all = imports(unit);
    let next = all.iter().find(|i| type_import(i).is_some_and(|(name, _)| name.as_str() > ZONE_ID));
    Some(match next {
        Some(next) => Edit::new(next.start..next.end, format!("import {ZONE_ID};\n{}", context.source(next))),
        None => {
            let last = all.last()?;
            Edit::new(last.start..last.end, format!("{}\nimport {ZONE_ID};", context.source(last)))
        }
    })
}
