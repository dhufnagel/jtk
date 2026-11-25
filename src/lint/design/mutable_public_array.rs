//! A059: a `public static final` field holding a non-empty array (Error Prone MutablePublicArray, IntelliJ
//! PublicStaticArrayField): the array is no constant, as any code can change its elements.
//!
//! No fix: the replacement (an unmodifiable list, or a method returning a copy) changes the API. Like Error Prone,
//! only arrays created in the initializer count (`{1}`, `new int[] {1}`, `new int[2]`; a length other than a literal
//! may be a constant `0`), and fields annotated with JUnit 4's `@DataPoints` are left out. Fields of interfaces are
//! implicitly `public static final`. IntelliJ's PublicStaticCollectionField needs types and is not covered.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, LitKind, Modifier, Tree, VarDecl};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Vars(vars) = &tree.kind else { return };
    let Some(K::Class(class)) = context.parent().map(|p| &p.kind) else { return };
    if !class.members.iter().any(|m| std::ptr::eq(m, tree)) {
        return;
    }
    let in_interface = matches!(class.kind, ClassKind::Interface | ClassKind::Annotation);
    for var in vars {
        let mods = &var.mods;
        let constant =
            in_interface || mods.has(Modifier::Public) && mods.has(Modifier::Static) && mods.has(Modifier::Final);
        let non_empty = var.init.as_deref().is_some_and(|init| is_non_empty_array(init, context.text));
        if constant && non_empty && !is_data_points(var) {
            let message = "public static final array is mutable";
            diagnostics.push(Diagnostic::new(Rule::MutablePublicArray, var.name_pos, message));
        }
    }
}

fn is_data_points(var: &VarDecl) -> bool {
    var.mods.annotations.iter().any(|a| match &a.kind {
        K::Annotation { ty, .. } => match &ty.kind {
            K::Ident(name) | K::MemberSelect { name, .. } => name == "DataPoints",
            _ => false,
        },
        _ => false,
    })
}

/// Whether the expression creates an array with elements: an initializer that is not empty, or a first dimension
/// that is an integer literal other than `0` (other lengths may be constants that are `0`).
fn is_non_empty_array(init: &Tree, text: &str) -> bool {
    let K::NewArray { dims, init, .. } = &init.kind else { return false };
    match init {
        Some(elements) => !elements.is_empty(),
        None => dims.first().is_some_and(|d| {
            matches!(d.kind, K::Literal(LitKind::Int))
                && text[d.start..d.end].bytes().any(|b| (b'1'..=b'9').contains(&b))
        }),
    }
}
