//! K043: an update of a volatile field that reads and writes it (`n++`, `n += 1`, `n = n + 1`) is not atomic:
//! concurrent updates get lost (IntelliJ NonAtomicOperationOnVolatileField, Error Prone NonAtomicVolatileUpdate).
//!
//! No fix: an atomic class (`AtomicInteger`) or a lock is needed.
//!
//! Only fields declared in the file are checked, by name or on `this`. Reported are increments and decrements,
//! compound assignments (all operators, like IntelliJ) and assignments whose value is a binary operation with the field
//! as an operand (`n = n * 2`, `n = 1 + n`). Not reported inside `synchronized` blocks and methods (as in both
//! originals), but in lambdas and classes declared there.

use super::shared::{Variable, code_boundary, enclosing_in_same_code, normalized, variable_below};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Modifier, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let target = match &tree.kind {
        K::Unary { op, expr, .. } if op == "++" || op == "--" => strip_parens(expr),
        K::Assign { op, lhs, .. } if op != "=" => strip_parens(lhs),
        K::Assign { lhs, rhs, .. } if reads(rhs, lhs, context) => strip_parens(lhs),
        _ => return,
    };
    if context.parent().is_some_and(|p| matches!(p.kind, K::Annotation { .. })) || in_synchronized(tree, context) {
        return;
    }
    let Some(field) = volatile_field(tree, target, context) else { return };
    let message = format!("Non-atomic update of volatile field '{}'", field.decl.name);
    diagnostics.push(Diagnostic::new(Rule::NonAtomicVolatile, tree.start, message));
}

/// Whether the value is a binary operation with the variable as one of its operands (through nested operations).
fn reads(value: &Tree, variable: &Tree, context: &Context) -> bool {
    match &strip_parens(value).kind {
        K::Binary { lhs, rhs, .. } => {
            let is_variable = |operand: &Tree| {
                let operand = strip_parens(operand);
                matches!(operand.kind, K::Ident(_) | K::MemberSelect { .. })
                    && normalized(operand, context) == normalized(strip_parens(variable), context)
            };
            is_variable(lhs) || is_variable(rhs) || reads(lhs, variable, context) || reads(rhs, variable, context)
        }
        _ => false,
    }
}

fn volatile_field<'t>(root: &'t Tree, target: &'t Tree, context: &Context<'t>) -> Option<Variable<'t>> {
    variable_below(root, target, context).filter(|v| v.is_field() && v.decl.mods.has(Modifier::Volatile))
}

/// Whether the tree is in a `synchronized` block or method, in the same code.
fn in_synchronized(tree: &Tree, context: &Context) -> bool {
    let in_block = enclosing_in_same_code(tree, context)
        .any(|(parent, child)| matches!(&parent.kind, K::Synchronized { block, .. } if std::ptr::eq(&**block, child)));
    in_block
        || code_boundary(tree, context)
            .is_some_and(|b| matches!(&b.kind, K::Method(method) if method.mods.has(Modifier::Synchronized)))
}
