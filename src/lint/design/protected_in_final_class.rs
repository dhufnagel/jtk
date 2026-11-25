//! A044: a `protected` field, method or constructor in a `final` class (PMD ProtectedMemberInFinalClass,
//! AvoidProtectedFieldInFinalClass and AvoidProtectedMethodInFinalClassNotExtending, NetBeans "Protected member in
//! final class", Error Prone ProtectedMembersInFinalClass, IntelliJ ProtectedMemberInFinalClass): there is no subclass
//! to grant access to, so `protected` only widens access to the package, which the member has anyway.
//!
//! Safe fix: removes `protected`, making the member package-private. Without subclasses, `protected` and
//! package-private grant the same access from source code; only reflection sees the difference. A method may override
//! or hide an inherited `protected` one, which must not become less visible, so methods are only checked in classes
//! that extend nothing, without `@Override`, and except `clone()` and `finalize()` (inherited from `Object`).
//! Interfaces cannot declare `protected` methods, so implemented interfaces do not matter. Only classes with the
//! `final` keyword count (not records or enums), as in Error Prone.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{ClassKind, K, Modifier, Mods, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    if class.kind != ClassKind::Class || !class.mods.has(Modifier::Final) {
        return;
    }
    for member in &class.members {
        let mods = match &member.kind {
            K::Vars(vars) => vars.first().map(|v| &v.mods),
            K::Method(method) => {
                let inherited = matches!(method.name.as_str(), "clone" | "finalize") && method.params.is_empty();
                let may_override = method.ret.is_some() && (class.extends.is_some() || inherited);
                (!may_override && !has_override(&method.mods)).then_some(&method.mods)
            }
            _ => None,
        };
        let Some((_, keyword)) = mods.and_then(|m| m.keywords.iter().find(|(k, _)| *k == Modifier::Protected)) else {
            continue;
        };
        let diagnostic = Diagnostic::new(Rule::ProtectedInFinalClass, keyword.start, "protected member in final class");
        let text = &context.text[keyword.end..];
        let spaces = text.len() - text.trim_start_matches([' ', '\t']).len();
        diagnostics.push(diagnostic.with_fix(vec![Edit::new(keyword.start..keyword.end + spaces, "")]));
    }
}

fn has_override(mods: &Mods) -> bool {
    mods.annotations.iter().any(|a| match &a.kind {
        K::Annotation { ty, .. } => match &ty.kind {
            K::Ident(name) | K::MemberSelect { name, .. } => name == "Override",
            _ => false,
        },
        _ => false,
    })
}
