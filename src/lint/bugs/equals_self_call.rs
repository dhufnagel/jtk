//! B035: `equals(Object o)` that calls itself with its parameter, `equals(o)` or `this.equals(o)`, which recurses
//! until the stack overflows (Error Prone EqualsReference).
//!
//! Only a call with the parameter itself (declared as `Object`) is reported, which cannot resolve to an overload such
//! as `equals(Foo)`, and not if the parameter is assigned in the method; nor are calls in lambdas and in nested or
//! anonymous classes (a different `this`). No fix.

use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Modifier, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    let [parameter] = method.params.as_slice() else { return };
    let is_object =
        parameter.ty.as_deref().is_some_and(|ty| matches!(context.source(ty), "Object" | "java.lang.Object"));
    let Some(body) = method.body.as_deref() else { return };
    if method.name != "equals" || !is_object || method.mods.has(Modifier::Static) || is_assigned(body, &parameter.name)
    {
        return;
    }
    for_each_descendant(body, &mut |descendant, _| {
        if let K::MethodInvocation { select, args, .. } = &descendant.kind {
            let calls_itself = match &select.kind {
                K::Ident(name) => name == "equals",
                K::MemberSelect { expr, name } => {
                    name == "equals" && matches!(&expr.kind, K::Ident(this) if this == "this")
                }
                _ => false,
            };
            let is_parameter = |argument: &Tree| matches!(&argument.kind, K::Ident(name) if *name == parameter.name);
            let passes_parameter = matches!(args.as_slice(), [argument] if is_parameter(argument));
            if calls_itself && passes_parameter {
                diagnostics.push(Diagnostic::new(Rule::EqualsSelfCall, descendant.start, "equals() calls itself"));
            }
        }
        !matches!(descendant.kind, K::Lambda { .. } | K::Class(_) | K::NewClass { body: Some(_), .. })
    });
}

/// Whether the variable `name` is assigned (or incremented) somewhere in the tree.
fn is_assigned(tree: &Tree, name: &str) -> bool {
    let mut assigned = false;
    for_each_descendant(tree, &mut |descendant, _| {
        let target = match &descendant.kind {
            K::Assign { lhs, .. } => Some(lhs),
            K::Unary { op, expr, .. } if op == "++" || op == "--" => Some(expr),
            _ => None,
        };
        assigned |= target.is_some_and(|t| matches!(&t.kind, K::Ident(n) if n == name));
        true
    });
    assigned
}
