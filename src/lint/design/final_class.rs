//! A002: a class that cannot be extended, as it only has private constructors, but is not declared `final`
//! (Checkstyle FinalClass, PMD ClassWithOnlyPrivateConstructorsShouldBeFinal).
//!
//! As in Checkstyle, a class without constructors counts if it is private, and a class is not reported if a class or
//! anonymous class in the same file extends it. Like Checkstyle, the extended class is found by name: among the
//! classes whose qualified name ends with it, the one sharing the longest prefix with the type where the name
//! appears (the class's enclosing type for `extends`).

use super::{package_name, qualified_name};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Modifier, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(class) = &tree.kind else { return };
    if class.kind != ClassKind::Class {
        return;
    }
    if class.mods.has(Modifier::Final) || class.mods.has(Modifier::Abstract) {
        return;
    }
    let mut constructors = class.members.iter().filter_map(|m| match &m.kind {
        K::Method(method) if method.ret.is_none() => Some(method),
        _ => None,
    });
    let only_private = match constructors.next() {
        None => class.mods.has(Modifier::Private),
        Some(first) => [first].into_iter().chain(constructors).all(|c| c.mods.has(Modifier::Private)),
    };
    if !only_private || is_extended_in_file(tree, context.ancestors[0]) {
        return;
    }
    let message = format!("Class {} should be declared as final", class.name);
    diagnostics.push(Diagnostic::new(Rule::FinalClass, tree.start, message));
}

/// A name of a class that another class extends, and the qualified name of the type the name appears in.
struct Extension {
    name: Vec<String>,
    scope: Vec<String>,
}

/// The classes of a file, by qualified name, and the names of the classes they extend.
#[derive(Default)]
struct Hierarchy<'t> {
    /// The qualified name of the type being walked, as segments.
    scope: Vec<String>,
    declarations: Vec<(Vec<String>, &'t Tree)>,
    extensions: Vec<Extension>,
}

/// Whether a class or anonymous class in the file extends the class. Without types, the extended class is found by
/// its name: the class whose qualified name ends with it and that is the closest relative of the type where the
/// name appears (the longest common prefix of qualified names), or the least nested and then the first one among
/// equally close ones.
fn is_extended_in_file(class: &Tree, unit: &Tree) -> bool {
    let package = package_name(unit);
    let scope = package.split('.').filter(|s| !s.is_empty()).map(str::to_string).collect();
    let mut hierarchy = Hierarchy { scope, ..Hierarchy::default() };
    unit.for_each_child(&mut |child| collect(child, &mut hierarchy));
    hierarchy.extensions.iter().any(|e| resolve(e, &hierarchy.declarations).is_some_and(|d| std::ptr::eq(d, class)))
}

fn collect<'t>(tree: &'t Tree, hierarchy: &mut Hierarchy<'t>) {
    match &tree.kind {
        K::Class(class) => {
            if let Some(superclass) = &class.extends
                && class.kind == ClassKind::Class
            {
                let name = segments(&qualified_name(superclass));
                hierarchy.extensions.push(Extension { name, scope: hierarchy.scope.clone() });
            }
            hierarchy.scope.push(class.name.clone());
            if class.kind == ClassKind::Class {
                hierarchy.declarations.push((hierarchy.scope.clone(), tree));
            }
            tree.for_each_child(&mut |child| collect(child, hierarchy));
            hierarchy.scope.pop();
            return;
        }
        K::NewClass { body: Some(_), .. } => {
            hierarchy.extensions.push(Extension { name: created_type(tree), scope: hierarchy.scope.clone() });
        }
        _ => {}
    }
    tree.for_each_child(&mut |child| collect(child, hierarchy));
}

/// The class an extended name refers to.
fn resolve<'t>(extension: &Extension, declarations: &[(Vec<String>, &'t Tree)]) -> Option<&'t Tree> {
    let common_prefix = |name: &[String]| name.iter().zip(&extension.scope).take_while(|(a, b)| a == b).count();
    declarations
        .iter()
        .filter(|(name, _)| name.ends_with(&extension.name))
        .min_by_key(|(name, _)| (std::cmp::Reverse(common_prefix(name)), name.len()))
        .map(|(_, declaration)| *declaration)
}

/// The name of the class an anonymous class extends: for `outer.new Inner() {}`, the class name of `outer` if it is
/// created too, followed by `Inner`.
fn created_type(creation: &Tree) -> Vec<String> {
    let K::NewClass { outer, ident, .. } = &creation.kind else { return Vec::new() };
    let mut name = match outer.as_deref() {
        Some(outer @ Tree { kind: K::NewClass { .. }, .. }) => created_type(outer),
        _ => Vec::new(),
    };
    name.extend(segments(&qualified_name(ident)));
    name
}

fn segments(name: &str) -> Vec<String> {
    name.split('.').map(str::to_string).collect()
}
