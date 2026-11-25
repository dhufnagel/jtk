//! B101: a random number in [0, 1) cast to an integer type, as in `(int) Math.random() * 10` (IntelliJ
//! MathRandomCastToInt, NetBeans "Math.random() casted to int", Error Prone RandomCast): the cast binds tighter than
//! the multiplication, so the value is always 0. Probably `(int) (Math.random() * 10)` was meant.
//!
//! No fix. The random values recognised are `Math.random()` and `StrictMath.random()`, and `nextDouble()` and
//! `nextFloat()` without arguments on `ThreadLocalRandom.current()`, on `new Random()` and the like, and on variables
//! declared in the file as `Random`, `SecureRandom`, `ThreadLocalRandom` or `SplittableRandom`.

use super::shared_arithmetic::{is_call, variable_type};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

const RANDOMS: &[&str] = &[
    "Random",
    "java.util.Random",
    "SecureRandom",
    "java.security.SecureRandom",
    "ThreadLocalRandom",
    "java.util.concurrent.ThreadLocalRandom",
    "SplittableRandom",
];

pub fn check<'t>(tree: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let K::Cast { ty, expr } = &tree.kind else { return };
    if !matches!(&ty.kind, K::Primitive(p) if matches!(p.as_str(), "int" | "long" | "short" | "byte" | "char")) {
        return;
    }
    let expr = strip_parens(expr);
    let K::MethodInvocation { select, .. } = &expr.kind else { return };
    let K::MemberSelect { expr: receiver, .. } = &select.kind else { return };
    let receiver = strip_parens(receiver);
    let is_random = if is_call(expr, "random", 0) {
        matches!(context.source(receiver), "Math" | "java.lang.Math" | "StrictMath" | "java.lang.StrictMath")
    } else if is_call(expr, "nextDouble", 0) || is_call(expr, "nextFloat", 0) {
        is_random_generator(receiver, tree, context)
    } else {
        false
    };
    if is_random {
        let message = "A random number in [0, 1) cast to an integer type is always 0";
        diagnostics.push(Diagnostic::new(Rule::RandomCastToInt, tree.start, message));
    }
}

/// Whether the expression is a random number generator.
fn is_random_generator<'t>(expression: &'t Tree, tree: &'t Tree, context: &Context<'t>) -> bool {
    match &expression.kind {
        K::NewClass { ident, body: None, .. } => RANDOMS.contains(&context.source(ident)),
        K::MethodInvocation { select, args, .. } if args.is_empty() => match &select.kind {
            K::MemberSelect { expr, name } => {
                name == "current"
                    && matches!(context.source(expr), "ThreadLocalRandom" | "java.util.concurrent.ThreadLocalRandom")
            }
            _ => false,
        },
        K::Ident(_) => variable_type(context, tree, expression).is_some_and(|t| RANDOMS.contains(&t)),
        _ => false,
    }
}
