//! S028: `extends Object` on a class or as the only bound of a type parameter (PMD and IntelliJ ExtendsObject,
//! Error Prone ExtendsObject, IntelliJ TypeParameterExtendsObject): every class extends `Object` and every type
//! parameter is bounded by it anyway.
//!
//! The fix removes ` extends Object`, which changes nothing: neither the superclass nor the erasure. A bound of an
//! intersection (`<T extends Object & Comparable<T>>`) is left alone, as it sets the erasure. `Object` is only taken
//! for `java.lang.Object` if no type of that name is declared in the file or imported; a class `Object` in the same
//! package is not known. Wildcards (`? extends Object`) are not reported.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{ClassKind, K, Tree};

use super::shared_batch3::{refers_to, without_whitespace};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let bound = match &tree.kind {
        K::Class(class) if class.kind == ClassKind::Class => class.extends.as_deref(),
        K::TypeParameter { bounds, .. } if bounds.len() == 1 => bounds.first(),
        _ => None,
    };
    let Some(bound) = bound else { return };
    if !matches!(bound.kind, K::Ident(_) | K::MemberSelect { .. }) {
        return;
    }
    let written = without_whitespace(context.source(bound));
    if !refers_to(&written, "java.lang.Object", &context.ancestors, context) {
        return;
    }
    let tokens = context.tokens();
    let Some(keyword) = tokens.previous(bound.start).filter(|k| &context.text[k.clone()] == "extends") else { return };
    let Some(before) = tokens.previous(keyword.start) else { return };
    let diagnostic = Diagnostic::new(Rule::ExtendsObject, keyword.start, "Unnecessary 'extends Object'");
    let range = before.end..bound.end;
    diagnostics.push(if tokens.has_comment(range.clone()) {
        diagnostic
    } else {
        diagnostic.with_fix(vec![Edit::new(range, "")])
    });
}
