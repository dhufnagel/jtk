//! Lengths in lines, from the opening to the closing brace as in Checkstyle:
//!
//! - C006: a method or constructor body (Checkstyle MethodLength with `countEmpty=true`, so blank and comment lines
//!   count),
//! - C008: a lambda body (Checkstyle LambdaBodyLength; for an expression, from its first to its last line),
//! - C009: an anonymous class body (Checkstyle AnonInnerLength).

use tree_sitter::Node;

use super::{end_line, line, token};
use crate::lint::syntax::source;
use crate::lint::{Diagnostic, Rule, RuleOptions};

pub fn check_method(method: &Node, text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let max = options.method_length.max as usize;
    let Some(body) = method.child_by_field_name("body") else { return };
    let length = end_line(&body) - line(&body) + 1;
    if length > max {
        let name = method.child_by_field_name("name").map_or("", |n| source(&n, text));
        let message = format!("Method {name} length is {length} lines (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::MethodLength, method.start_byte(), message));
    }
}

pub fn check_lambda(lambda: &Node, _text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let max = options.lambda_body_length.max as usize;
    let Some(body) = lambda.child_by_field_name("body") else { return };
    let length = end_line(&body) - line(&body) + 1;
    if length > max {
        let arrow = token(lambda, "->").map_or(lambda.start_byte(), |a| a.start_byte());
        let message = format!("Lambda body length is {length} lines (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::LambdaBodyLength, arrow, message));
    }
}

pub fn check_anonymous_class(creation: &Node, _text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    let max = options.anon_inner_length.max as usize;
    let Some(body) = creation.named_children(&mut creation.walk()).find(|c| c.kind() == "class_body") else {
        return;
    };
    let length = end_line(&body) - line(&body) + 1;
    if length > max {
        let new = token(creation, "new").map_or(creation.start_byte(), |n| n.start_byte());
        let message = format!("Anonymous inner class length is {length} lines (max allowed is {max}).");
        diagnostics.push(Diagnostic::new(Rule::AnonInnerLength, new, message));
    }
}
