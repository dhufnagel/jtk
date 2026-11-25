//! B168: a collection or map passed to its own bulk method, `x.addAll(x)`, `x.removeAll(x)`, `x.retainAll(x)`,
//! `x.containsAll(x)` or `x.putAll(x)`: it duplicates, clears or does nothing, or is always true, so another argument
//! was meant (Error Prone ModifyingCollectionWithItself, IntelliJ CollectionAddedToSelf).
//!
//! Only these method names are checked, as for other methods (`x.add(x)` on a `BigDecimal`) passing the receiver
//! can be right; receiver and argument must be the same simple name or the same `this.field`. IntelliJ's
//! `list.add(list)` is not reported, for that reason. No fix.

use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

const BULK_METHODS: &[&str] = &["addAll", "removeAll", "retainAll", "containsAll", "putAll"];

pub fn check(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { select, args, .. } = &tree.kind else { return };
    let K::MemberSelect { expr: receiver, name } = &select.kind else { return };
    let [argument] = args.as_slice() else { return };
    if !BULK_METHODS.contains(&name.as_str()) {
        return;
    }
    let same = match (variable(receiver), variable(argument)) {
        (Some(receiver), Some(argument)) => receiver == argument,
        _ => false,
    };
    if same {
        let message = format!("{name}() with its own receiver as argument");
        diagnostics.push(Diagnostic::new(Rule::CollectionWithItself, tree.start, message));
    }
}

/// A simple name (not `this` or `super`), or `this.name` (marked by the `bool`).
fn variable(tree: &Tree) -> Option<(bool, &str)> {
    match &strip_parens(tree).kind {
        K::Ident(name) if name != "this" && name != "super" => Some((false, name)),
        K::MemberSelect { expr, name } if matches!(&expr.kind, K::Ident(this) if this == "this") => Some((true, name)),
        _ => None,
    }
}
