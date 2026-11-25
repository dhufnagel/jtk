//! S137: `new T[]` before the array initializer of a variable declared as `T[]` (IntelliJ
//! UnnecessaryConstantArrayCreationExpression, PMD UseShortArrayInitializer): `int[] a = new int[] {1, 2};` is
//! `int[] a = {1, 2};`.
//!
//! Only reported where the created type is written exactly as the declared one: `Object[] a = new String[] {}`
//! creates a `String[]`, and `var` needs the creation. Array initializers nested in others are not reported. The
//! fix removes `new T[]`, which changes nothing.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

use super::shared_batch3::without_whitespace;

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::NewArray { elem: Some(_), dims, init: Some(_), .. } = &tree.kind else { return };
    let Some(K::Vars(vars)) = context.parent().map(|p| &p.kind) else { return };
    let Some(var) = vars.iter().find(|v| v.init.as_deref().is_some_and(|i| std::ptr::eq(i, tree))) else { return };
    let Some(declared) = var.ty.as_deref().filter(|t| t.end <= var.name_pos) else { return };
    let tokens = context.tokens();
    let Some(brace) = tokens.in_range(tree.start..tree.end).find(|t| &context.text[t.clone()] == "{") else { return };
    let created = context.text[tree.start..brace.start].strip_prefix("new").unwrap_or_default();
    if !dims.is_empty() || without_whitespace(created) != without_whitespace(context.source(declared)) {
        return;
    }
    let diagnostic =
        Diagnostic::new(Rule::ArrayCreationInInitializer, tree.start, "Array creation can be an array initializer");
    let range = tree.start..brace.start;
    diagnostics.push(if tokens.has_comment(range.clone()) {
        diagnostic
    } else {
        diagnostic.with_fix(vec![Edit::new(range, "")])
    });
}
