//! Lengths in lines, from the opening to the closing brace as in Checkstyle:
//!
//! - C006: a method or constructor body (Checkstyle MethodLength with `countEmpty=true`, so blank and comment lines
//!   count),
//! - C008: a lambda body (Checkstyle LambdaBodyLength; for an expression, from its first to its last line),
//! - C009: an anonymous class body (Checkstyle AnonInnerLength).

use super::{line_span, token_after, token_before};
use crate::ast::{K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check_method(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    let Some(body) = &method.body else { return };
    let max = context.options.method_length.max as usize;
    let length = line_span(context.text, body.start, body.end);
    if length > max {
        let message = format!("Method {} length is {length} lines (max allowed is {max}).", method.name);
        diagnostics.push(Diagnostic::new(Rule::MethodLength, tree.start, message));
    }
}

pub fn check_lambda(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Lambda { body, .. } = &tree.kind else { return };
    let max = context.options.lambda_body_length.max as usize;
    let length = line_span(context.text, body.start, body.end);
    if length > max {
        let arrow = token_before(context, body.start, tree.start);
        let message = format!("Lambda body length is {length} lines (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::LambdaBodyLength, arrow, message));
    }
}

pub fn check_anonymous_class(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::NewClass { outer, ident, args, body: Some(_), .. } = &tree.kind else { return };
    let max = context.options.anon_inner_length.max as usize;
    // The whole expression is short enough most of the time, then the body need not be found.
    if line_span(context.text, tree.start, tree.end) <= max {
        return;
    }
    // The body starts after the `)` of the arguments.
    let tokens = context.tokens();
    let close = match args.last() {
        Some(last) => tokens.next(last.end),
        None => tokens.next(ident.end).and_then(|open| tokens.next(open.end)),
    };
    let Some(open) = close.and_then(|close| tokens.next(close.end)) else { return };
    let length = line_span(context.text, open.start, tree.end);
    if length > max {
        // `new`, after `outer.` if there is an outer instance.
        let new = match outer {
            Some(outer) => {
                let dot = tokens.next(outer.end).map_or(outer.end, |t| t.end);
                token_after(context, dot, tree.start)
            }
            None => tree.start,
        };
        let message = format!("Anonymous inner class length is {length} lines (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::AnonInnerLength, new, message));
    }
}
