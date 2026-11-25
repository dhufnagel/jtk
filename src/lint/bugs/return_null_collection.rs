//! B174: `return null;` in a method that returns a collection (PMD ReturnEmptyCollectionRatherThanNull, Error Prone
//! ReturnsNullCollection, Sonar java:S1168): callers must check for null, where an empty collection would just work.
//!
//! Unsafe fix: `List.of()`, `Set.of()` or `Map.of()` (Java 9); for `Collection`, `List.of()` if `List` is
//! `java.util.List` in the file. Callers that check for null or modify the result behave differently.
//!
//! Narrower than the originals: only the declared return types `List`, `Set`, `Map` and `Collection` of `java.util`
//! (through the file's imports, and not declared in the file, or written out), not arrays or other collection types,
//! and only a literal `null`. Returns in lambdas are skipped, as are methods annotated `@Nullable` or
//! `@CheckForNull` (of any package), which declare that null is a valid result.

use crate::lint::logging::shared::{declares_type, imports_type, qualified_name};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, LitKind, MethodDecl, Tree};

/// The collection types of `java.util` with the empty collection to return instead of null, if any.
const COLLECTIONS: &[(&str, Option<&str>)] =
    &[("List", Some("List")), ("Set", Some("Set")), ("Map", Some("Map")), ("Collection", None)];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Return(Some(expression)) = &tree.kind else { return };
    if !matches!(strip_parens(expression).kind, K::Literal(LitKind::Null)) {
        return;
    }
    let Some(unit) = context.ancestors.first() else { return };
    let enclosing = context.ancestors.iter().rev().find(|t| matches!(t.kind, K::Method(_) | K::Lambda { .. }));
    let Some(K::Method(method)) = enclosing.map(|t| &t.kind) else { return };
    let Some(ret) = method.ret.as_deref() else { return };
    if is_nullable(method) {
        return;
    }
    let mut base = ret;
    while let K::Parameterized { base: inner, .. } | K::Annotated { base: inner, .. } = &base.kind {
        base = inner;
    }
    let (simple, qualified) = match &base.kind {
        K::Ident(name) => (name.as_str(), false),
        K::MemberSelect { name, .. } => (name.as_str(), true),
        _ => return,
    };
    let Some((_, empty)) = COLLECTIONS.iter().find(|(name, _)| *name == simple) else { return };
    let resolves =
        |simple: &str| imports_type(unit, simple, &format!("java.util.{simple}")) && !declares_type(unit, simple);
    if qualified && qualified_name(base) != format!("java.util.{simple}") || !qualified && !resolves(simple) {
        return;
    }
    let mut diagnostic =
        Diagnostic::new(Rule::ReturnNullCollection, tree.start, "null returned instead of an empty collection");
    let factory = match empty {
        Some(factory) => Some(*factory),
        None if qualified || resolves("List") => Some("List"),
        None => None,
    };
    if let Some(factory) = factory
        && !context.tokens().has_comment(expression.start..expression.end)
    {
        let prefix = if qualified { "java.util." } else { "" };
        diagnostic = diagnostic
            .with_unsafe_fix(vec![Edit::new(expression.start..expression.end, format!("{prefix}{factory}.of()"))]);
    }
    diagnostics.push(diagnostic);
}

/// Whether the method or its return type is annotated `@Nullable` or `@CheckForNull`.
fn is_nullable(method: &MethodDecl) -> bool {
    let mut annotations: Vec<&Tree> = method.mods.annotations.iter().chain(&method.return_type_annotations).collect();
    let mut ty = method.ret.as_deref();
    while let Some(tree) = ty {
        ty = match &tree.kind {
            K::Annotated { annotations: on_type, base } => {
                annotations.extend(on_type);
                Some(base)
            }
            K::Parameterized { base, .. } => Some(base),
            _ => None,
        };
    }
    annotations.iter().any(|annotation| {
        let K::Annotation { ty, .. } = &annotation.kind else { return false };
        let name = qualified_name(ty);
        let simple = name.rsplit('.').next().unwrap_or_default();
        simple == "Nullable" || simple == "CheckForNull"
    })
}
