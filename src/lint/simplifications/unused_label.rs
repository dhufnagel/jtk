//! S059: a label that no `break` or `continue` names (IntelliJ, Error Prone and PMD UnusedLabel, Eclipse unused
//! label).
//!
//! The fix removes the label with the colon, which changes nothing: a label can only be used inside its statement.

use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Labeled { label, body } = &tree.kind else { return };
    let names_label = |t: &Tree| matches!(&t.kind, K::Break(Some(l)) | K::Continue(Some(l)) if l == label);
    let mut used = names_label(body);
    for_each_descendant(body, &mut |t, _| {
        used |= names_label(t);
        !used
    });
    if used {
        return;
    }
    let diagnostic = Diagnostic::new(Rule::UnusedLabel, tree.start, format!("Unused label '{label}'"));
    let range = tree.start..body.start;
    diagnostics.push(if context.tokens().has_comment(range.clone()) {
        diagnostic
    } else {
        diagnostic.with_fix(vec![Edit::new(range, "")])
    });
}
