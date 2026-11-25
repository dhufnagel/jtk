//! T033: `assertEquals` on two arrays (IntelliJ AssertEqualsCalledOnArray, NetBeans DN_assertEqualsForArrays):
//! arrays do not override `equals`, so it compares their identity, not their elements.
//!
//! Only where both compared values are arrays of the same type known locally: array creations (`new int[] {1}`) or
//! variables, parameters and fields declared with an array type in the file. JUnit 5 always compares arrays by
//! identity; JUnit 4's deprecated `assertEquals(Object[], Object[])` compares the elements, so for JUnit 4 only
//! arrays of a primitive type are reported. The assertion is recognised by the imports or the class it is called on.
//!
//! Unsafe fix: call `assertArrayEquals` (importing it like `assertEquals` if needed), which passes for equal
//! elements where `assertEquals` failed. Not for JUnit 4 arrays of `float` or `double` without a delta, for which
//! JUnit 4 has no `assertArrayEquals`.

use super::shared::{Framework, Names, compared_values, local_type, path_with};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

const PRIMITIVES: &[&str] = &["boolean", "byte", "char", "short", "int", "long", "float", "double"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { args, .. } = &tree.kind else { return };
    if args.len() < 2 || super::shared::method_name(tree) != Some("assertEquals") {
        return;
    }
    let Some(names) = Names::new(context) else { return };
    let Some((framework, _)) = names.assertion(tree, &context.ancestors) else { return };
    if !matches!(framework, Framework::JUnit4 | Framework::JUnit5) {
        return;
    }
    let Some((first, second)) = compared_values(framework, args) else { return };
    let path = path_with(context, tree);
    let types = (local_type(&args[first], &path, context.text), local_type(&args[second], &path, context.text));
    let (Some(a), Some(b)) = types else { return };
    let Some(element) = a.strip_suffix("[]") else { return };
    let primitive = PRIMITIVES.contains(&element);
    if a != b || framework == Framework::JUnit4 && !primitive {
        return;
    }
    let diagnostic = Diagnostic::new(Rule::AssertEqualsOnArrays, tree.start, "assertEquals() on arrays");
    // JUnit 4 compares arrays of floating point numbers only with a delta.
    let needs_delta =
        framework == Framework::JUnit4 && matches!(element, "float" | "double") && args.len() == second + 1;
    let class = names.static_method_class(tree, &context.ancestors).unwrap_or_default();
    let fix = (!needs_delta).then(|| names.rename_static_call(tree, &class, "assertArrayEquals")).flatten();
    diagnostics.push(match fix {
        Some(fix) => diagnostic.with_unsafe_fix(fix),
        None => diagnostic,
    });
}
