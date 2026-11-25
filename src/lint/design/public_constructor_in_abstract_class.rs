//! A020: a `public` constructor in an abstract class (IntelliJ NonProtectedConstructorInAbstractClass, Sonar
//! java:S5993). Only subclasses can call it, so `public` promises more than it gives.
//!
//! Unsafe fix: `public` becomes `protected`. Code in the same package and subclasses can still call it, but
//! reflection (`Class.getConstructor()`) and frameworks that look only for public constructors no longer find it.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{ClassKind, K, Modifier, Tree};

pub fn check(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    if class.kind != ClassKind::Class || !class.mods.has(Modifier::Abstract) {
        return;
    }
    for member in &class.members {
        let K::Method(method) = &member.kind else { continue };
        if method.ret.is_some() {
            continue;
        }
        let Some((_, public)) = method.mods.keywords.iter().find(|(m, _)| *m == Modifier::Public) else { continue };
        let message = "Public constructor in an abstract class";
        diagnostics.push(
            Diagnostic::new(Rule::PublicConstructorInAbstractClass, member.start, message)
                .with_unsafe_fix(vec![Edit::new(public.clone(), "protected")]),
        );
    }
}
