//! S030: a constructor that the compiler would generate anyway (PMD UnnecessaryConstructor, IntelliJ
//! UnnecessaryConstructor): the only constructor of a class, without parameters, type parameters, annotations,
//! `throws` or statements, with the same access as the class (Java's default constructor has the access of the
//! class).
//!
//! Safe fix: remove the constructor; the default constructor that replaces it is the same.
//!
//! Narrower than the originals: enums and records are skipped, as are classes in interfaces (implicitly public, so
//! the default constructor is public too), constructors with comments in or right before them (a Javadoc documents
//! them), and classes with a Lombok annotation that generates constructors (`@AllArgsConstructor`, `@Data`, ...):
//! without the explicit constructor, the class would lose its no-argument one. Constructors that only call
//! `super()` are left to S011.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Modifier, Mods, Tree};

use super::line_removal;

/// The Lombok annotations that generate constructors, by simple name.
const LOMBOK_CONSTRUCTORS: &[&str] =
    &["AllArgsConstructor", "RequiredArgsConstructor", "NoArgsConstructor", "Builder", "SuperBuilder", "Data", "Value"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    if class.kind != ClassKind::Class || in_interface(context) || has_lombok_constructors(&class.mods) {
        return;
    }
    let mut constructors =
        class.members.iter().filter(|m| matches!(&m.kind, K::Method(method) if method.ret.is_none()));
    let (Some(constructor), None) = (constructors.next(), constructors.next()) else { return };
    let K::Method(method) = &constructor.kind else { return };
    let is_empty = matches!(method.body.as_deref().map(|b| &b.kind), Some(K::Block { stmts, .. }) if stmts.is_empty());
    if !is_empty
        || !method.params.is_empty()
        || method.receiver.is_some()
        || !method.type_params.is_empty()
        || !method.throws.is_empty()
        || !method.mods.annotations.is_empty()
        || access(&method.mods) != access(&class.mods)
    {
        return;
    }
    let tokens = context.tokens();
    let before = tokens.previous(constructor.start).map_or(0, |token| token.end);
    if tokens.has_comment(before..constructor.end) {
        return;
    }
    let message = "Unnecessary constructor: the compiler generates the same default constructor";
    diagnostics.push(
        Diagnostic::new(Rule::UnnecessaryConstructor, constructor.start, message)
            .with_fix(vec![line_removal(constructor.start..constructor.end, context.text)]),
    );
}

/// The access modifier, if any.
fn access(mods: &Mods) -> Option<Modifier> {
    [Modifier::Public, Modifier::Protected, Modifier::Private].into_iter().find(|&m| mods.has(m))
}

/// Whether the checked class is a member of an interface or annotation type.
fn in_interface(context: &Context) -> bool {
    matches!(context.parent().map(|p| &p.kind), Some(K::Class(outer)) if matches!(outer.kind, ClassKind::Interface | ClassKind::Annotation))
}

fn has_lombok_constructors(mods: &Mods) -> bool {
    mods.annotations.iter().any(|annotation| {
        let K::Annotation { ty, .. } = &annotation.kind else { return false };
        let name = match &ty.kind {
            K::Ident(name) | K::MemberSelect { name, .. } => name,
            _ => return false,
        };
        LOMBOK_CONSTRUCTORS.contains(&name.as_str())
    })
}
