//! A011: an abstract class without abstract methods, which neither extends a class nor implements an interface (PMD
//! AbstractClassWithoutAbstractMethod). A class that has supertypes may leave their abstract methods unimplemented.

use tree_sitter::Node;

use super::{has_keyword, members};
use crate::lint::{Diagnostic, Rule};

pub fn check(class: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if !has_keyword(class, text, "abstract")
        || class.child_by_field_name("superclass").is_some()
        || class.child_by_field_name("interfaces").is_some()
    {
        return;
    }
    let Some(body) = class.child_by_field_name("body") else { return };
    let has_abstract_method =
        members(&body).iter().any(|m| m.kind() == "method_declaration" && has_keyword(m, text, "abstract"));
    if !has_abstract_method {
        let name = class.child_by_field_name("name").unwrap_or(*class);
        let message = "This abstract class does not have any abstract methods";
        diagnostics.push(Diagnostic::new(Rule::AbstractWithoutAbstract, name.start_byte(), message));
    }
}
