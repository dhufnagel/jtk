//! A005: an interface with constants but no methods, used to share constants rather than to describe a type
//! (Checkstyle InterfaceIsType, PMD ConstantsInInterface). Marker interfaces without members are allowed.

use tree_sitter::Node;

use super::{body, members};
use crate::lint::{Diagnostic, Rule};

pub fn check(interface: &Node, _text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(body) = body(interface) else { return };
    let members = members(&body);
    let has_method = members.iter().any(|m| m.kind() == "method_declaration");
    let has_constant = members.iter().any(|m| m.kind() == "constant_declaration");
    if has_constant && !has_method {
        let message = "Interfaces should describe a type and hence have methods";
        diagnostics.push(Diagnostic::new(Rule::ConstantInterface, interface.start_byte(), message));
    }
}
