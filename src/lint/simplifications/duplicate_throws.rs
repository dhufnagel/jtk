//! S036: an exception listed twice in a `throws` clause (IntelliJ DuplicateThrows).
//!
//! The fix removes the repetition with the comma before it, which changes nothing. Only the same name written the
//! same way counts: `IOException` and `java.io.IOException`, or an exception and its subclass, are not compared.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

use super::shared_batch3::without_whitespace;

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    let names: Vec<String> = method.throws.iter().map(|t| without_whitespace(context.source(t))).collect();
    let tokens = context.tokens();
    for (index, thrown) in method.throws.iter().enumerate() {
        if !names[..index].contains(&names[index]) {
            continue;
        }
        let message = format!("Duplicate exception '{}' in throws clause", names[index]);
        let diagnostic = Diagnostic::new(Rule::DuplicateThrows, thrown.start, message);
        let comma = tokens.previous(thrown.start).filter(|c| &context.text[c.clone()] == ",");
        diagnostics.push(match comma {
            Some(comma) if !tokens.has_comment(comma.start..thrown.end) => {
                diagnostic.with_fix(vec![Edit::new(comma.start..thrown.end, "")])
            }
            _ => diagnostic,
        });
    }
}
