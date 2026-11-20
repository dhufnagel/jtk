//! A005: an interface with constants but no methods, used to share constants rather than to describe a type
//! (Checkstyle InterfaceIsType, PMD ConstantsInInterface). Marker interfaces without members are allowed.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Tree};

pub fn check(tree: &Tree, _context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Class(interface) = &tree.kind else { return };
    if interface.kind != ClassKind::Interface {
        return;
    }
    let has_method = interface.members.iter().any(|m| matches!(m.kind, K::Method(_)));
    let has_constant = interface.members.iter().any(|m| matches!(m.kind, K::Vars(_)));
    if has_constant && !has_method {
        let message = "Interfaces should describe a type and hence have methods";
        diagnostics.push(Diagnostic::new(Rule::ConstantInterface, tree.start, message));
    }
}
