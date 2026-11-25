//! N015: a method named like its class (PMD MethodWithSameNameAsEnclosingClass, Eclipse
//! COMPILER_PB_METHOD_WITH_CONSTRUCTOR_NAME, IntelliJ MethodNameSameAsClassName): mostly a constructor that got a
//! return type by mistake, so it is never called on `new`.
//!
//! No fix: whether a constructor was meant is a guess. Methods in the bodies of enum constants and anonymous classes
//! are not checked.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    let Some(K::Class(class)) = context.parent().map(|p| &p.kind) else { return };
    let is_member = class.members.iter().any(|m| std::ptr::eq(m, tree));
    if is_member && method.ret.is_some() && method.name == class.name {
        diagnostics.push(Diagnostic::new(Rule::MethodNamedLikeClass, method.name_pos, "method named like its class"));
    }
}
