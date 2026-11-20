//! S006: parentheses that change nothing (Checkstyle UnnecessaryParentheses, PMD UselessParentheses, Error Prone
//! UnnecessaryParentheses): around a name or literal, a whole value (returned, assigned, passed as argument, lambda
//! body, `case` label, guard or assertion), a single lambda parameter, other parentheses, or an operation that binds
//! tighter than the one around it (`(a && b) || c`). Like Checkstyle, associativity is not considered: `(a + b) + c`
//! is not reported.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    match &tree.kind {
        K::Parens(inner) => check_parentheses(tree, inner, context, diagnostics),
        K::Lambda { params, .. } => check_lambda_parameter(tree, params, context, diagnostics),
        _ => {}
    }
}

fn check_parentheses(parentheses: &Tree, inner: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let Some(parent) = context.parent() else { return };
    // Comments inside the inner expression stay with it.
    let tokens = context.tokens();
    let has_comment =
        tokens.has_comment(parentheses.start..inner.start) || tokens.has_comment(inner.end..parentheses.end);
    if has_comment || is_syntax(parentheses, parent) {
        return;
    }
    let is = |t: &Tree| std::ptr::eq(t, parentheses);
    let message = match &inner.kind {
        K::Ident(name) if name != "this" && name != "super" => {
            format!("Unnecessary parentheses around identifier '{name}'")
        }
        K::Literal(_) if !is_negative_literal(inner, context) => {
            format!("Unnecessary parentheses around literal '{}'", context.source(inner))
        }
        _ if matches!(parent.kind, K::Parens(_)) => "Unnecessary parentheses around expression".to_string(),
        _ if is_whole_value(parent, &is, context) => "Unnecessary parentheses around a whole value".to_string(),
        _ if binds_tighter(inner, parent, &is, context) => "Unnecessary parentheses around expression".to_string(),
        _ => return,
    };
    let fix = Edit::new(parentheses.start..parentheses.end, context.source(inner));
    diagnostics.push(Diagnostic::new(Rule::UnnecessaryParentheses, parentheses.start, message).with_fix(vec![fix]));
}

/// Whether the parentheses belong to the syntax of a statement or `switch`: `if (c)`, `switch (x)`.
fn is_syntax(parentheses: &Tree, parent: &Tree) -> bool {
    let is = |t: &Tree| std::ptr::eq(t, parentheses);
    match &parent.kind {
        K::If { .. } | K::While { .. } | K::DoWhile { .. } | K::Synchronized { .. } => true,
        K::Switch { selector, .. } | K::SwitchExpr { selector, .. } => is(selector),
        _ => false,
    }
}

/// `-1` is a literal in the AST, but an operation in tree-sitter's syntax tree, whose behaviour the rule keeps.
fn is_negative_literal(tree: &Tree, context: &Context) -> bool {
    matches!(tree.kind, K::Literal(_)) && context.source(tree).starts_with('-')
}

/// The whole value of a `return`, `yield`, variable initializer, assignment, argument, lambda body, `case` label,
/// guard or assertion.
fn is_whole_value(parent: &Tree, is: &impl Fn(&Tree) -> bool, context: &Context) -> bool {
    match &parent.kind {
        K::Return(_) | K::Yield(_) | K::Assert { .. } | K::Vars(_) | K::Var(_) => true,
        K::MethodInvocation { args, .. } | K::NewClass { args, .. } => args.iter().any(is),
        K::Class(class) => class.enum_constants.iter().any(|c| c.args.iter().any(is)),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            cases.iter().any(|c| c.labels.iter().any(is) || c.guard.as_deref().is_some_and(is))
        }
        K::Lambda { body, .. } => is(body),
        K::For { cond, .. } => cond.as_deref().is_some_and(is),
        // An annotation's `name = value` is not an assignment.
        K::Assign { rhs, .. } => {
            is(rhs) && !context.grandparent().is_some_and(|g| matches!(g.kind, K::Annotation { .. }))
        }
        _ => false,
    }
}

/// Operations whose parentheses Checkstyle reports by default when they bind tighter than the operation around
/// them; arithmetic and bitwise operations are not reported.
const CONDITIONAL_OPERATORS: &[&str] = &["&&", "||", "==", "!=", "<", ">", "<=", ">="];

/// Expressions that bind tighter than any operator: calls, member and array accesses, casts and `this`.
fn is_primary(tree: &Tree) -> bool {
    match &tree.kind {
        K::MethodInvocation { .. } | K::ArrayAccess { .. } | K::Cast { .. } => true,
        K::MemberSelect { name, .. } => name != "class",
        K::Ident(name) => name == "this",
        _ => false,
    }
}

/// Whether the parenthesized expression binds tighter than the operation around it: a comparison or logical
/// operation inside `&&` or `||`, a unary operation inside a comparison or logical operation, or a call or member
/// access inside any operation (a cast too, unless a member of it is selected). The condition of `?:` may keep its
/// parentheses for clarity.
fn binds_tighter(inner: &Tree, parent: &Tree, is: &impl Fn(&Tree) -> bool, context: &Context) -> bool {
    let outer = match &parent.kind {
        K::Binary { op, .. } => op.as_str(),
        K::Conditional { cond, .. } => return !is(cond) && is_primary(inner),
        K::Unary { op, .. } => return !matches!(op.as_str(), "++" | "--") && is_primary(inner),
        K::Cast { .. } => return is_primary(inner),
        // `(outer).super(...)` is a constructor invocation in tree-sitter's syntax tree, not a member access.
        K::MemberSelect { name, .. } => {
            return name != "super" && is_primary(inner) && !matches!(inner.kind, K::Cast { .. });
        }
        _ => return false,
    };
    let is_conditional_context = CONDITIONAL_OPERATORS.contains(&outer);
    match &inner.kind {
        K::Binary { op, .. } => {
            // `p == (q <= r)` may need its parentheses: comparing booleans is not reported.
            matches!(outer, "&&" | "||")
                && CONDITIONAL_OPERATORS.contains(&op.as_str())
                && (precedence(op) > precedence(outer) || op == outer)
        }
        K::InstanceOf { .. } => is_conditional_context && precedence("instanceof") > precedence(outer),
        K::Unary { .. } => is_conditional_context,
        K::Literal(_) => is_conditional_context && is_negative_literal(inner, context),
        _ => is_primary(inner),
    }
}

fn precedence(operator: &str) -> u8 {
    match operator {
        "||" => 4,
        "&&" => 5,
        "|" => 6,
        "^" => 7,
        "&" => 8,
        "==" | "!=" => 9,
        "<" | ">" | "<=" | ">=" | "instanceof" => 10,
        "<<" | ">>" | ">>>" => 11,
        "+" | "-" => 12,
        _ => 13,
    }
}

/// `(x) -> ...`: a single parameter without type needs no parentheses.
fn check_lambda_parameter(
    lambda: &Tree,
    params: &[jtk_parser::ast::VarDecl],
    context: &Context,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let [parameter] = params else { return };
    if parameter.ty.is_some() || !context.text[lambda.start..].starts_with('(') {
        return;
    }
    let Some(close) = context.tokens().next(parameter.end) else { return };
    let fix = Edit::new(lambda.start..close.end, parameter.name.as_str());
    let message = "Unnecessary parentheses around lambda parameter";
    diagnostics.push(Diagnostic::new(Rule::UnnecessaryParentheses, lambda.start, message).with_fix(vec![fix]));
}
