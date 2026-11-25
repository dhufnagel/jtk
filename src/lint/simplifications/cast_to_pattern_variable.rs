//! S102: a cast of a variable to the type of a pattern variable that holds it already (IntelliJ
//! CastCanBeReplacedWithVariable): in `if (o instanceof String s) ((String) o).length();` the cast is `s`.
//!
//! Only reported where the pattern variable is certainly in scope and holds the same value: the cast is in the
//! branch that runs when an `instanceof` with a type pattern is true (the `then` of an `if` or `?:`, the body of a
//! `while`, the right operand of `&&`, also through further `&&` operands), not in another class, the cast type is
//! written as in the pattern, and the variable is a local variable or parameter (a field could change in between)
//! that, like the pattern variable, is not assigned anywhere in the method.
//!
//! The fix writes the pattern variable instead of the cast and the parentheses around it, which changes nothing.

use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

use super::shared_batch3::{is_assigned_in, is_local_variable, without_whitespace};

pub fn check(cast: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Cast { ty, expr } = &cast.kind else { return };
    let K::Ident(variable) = &expr.kind else { return };
    let ty = without_whitespace(context.source(ty));
    let Some(pattern) = pattern_variable(cast, variable, &ty, context) else { return };
    let ancestors = &context.ancestors;
    let Some(code) = ancestors.iter().rev().find(|a| matches!(a.kind, K::Method(_) | K::Lambda { .. } | K::Class(_)))
    else {
        return;
    };
    if is_assigned_in(code, variable)
        || is_assigned_in(code, pattern)
        || !is_local_variable(cast, ancestors, variable, context)
    {
        return;
    }
    // The cast with the parentheses around it.
    let outermost =
        ancestors.iter().rev().take_while(|a| matches!(a.kind, K::Parens(_))).last().copied().unwrap_or(cast);
    let message = format!("Cast can be replaced with pattern variable '{pattern}'");
    let diagnostic = Diagnostic::new(Rule::CastToPatternVariable, cast.start, message);
    diagnostics.push(if context.tokens().has_comment(outermost.start..outermost.end) {
        diagnostic
    } else {
        diagnostic.with_fix(vec![Edit::new(outermost.start..outermost.end, pattern)])
    });
}

/// The name of the pattern variable in scope at the cast that holds `variable` as type `ty`.
fn pattern_variable<'t>(cast: &Tree, variable: &str, ty: &str, context: &Context<'t>) -> Option<&'t str> {
    let mut child = cast;
    for parent in context.ancestors.iter().rev() {
        let condition = match &parent.kind {
            K::If { cond, then, .. } | K::Conditional { cond, then, .. } if std::ptr::eq(&**then, child) => Some(cond),
            K::While { cond, body } if std::ptr::eq(&**body, child) => Some(cond),
            K::Binary { op, lhs, rhs } if op == "&&" && std::ptr::eq(&**rhs, child) => Some(lhs),
            K::Class(_) | K::NewClass { .. } => return None,
            _ => None,
        };
        if let Some(found) = condition.and_then(|c| binding_when_true(c, variable, ty, context)) {
            return Some(found);
        }
        child = parent;
    }
    None
}

/// The pattern variable for `variable instanceof ty name` that the condition binds when it is true.
fn binding_when_true<'t>(condition: &'t Tree, variable: &str, ty: &str, context: &Context) -> Option<&'t str> {
    match &strip_parens(condition).kind {
        K::InstanceOf { expr, target } => {
            let K::Ident(name) = &strip_parens(expr).kind else { return None };
            let K::BindingPattern { var } = &target.kind else { return None };
            let same_type = var.ty.as_deref().is_some_and(|t| without_whitespace(context.source(t)) == ty);
            (name == variable && same_type && var.name != "_").then_some(var.name.as_str())
        }
        K::Binary { op, lhs, rhs } if op == "&&" => {
            binding_when_true(lhs, variable, ty, context).or_else(|| binding_when_true(rhs, variable, ty, context))
        }
        _ => None,
    }
}
