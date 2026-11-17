//! A001: a utility class (only static members, at least one of them not private) with a public or default
//! constructor (Checkstyle HideUtilityClassConstructor, PMD InstantiableUtilityClass).
//!
//! As in Checkstyle, abstract and `static` classes and classes that extend another class are not utility classes,
//! and a protected constructor hides the constructor too.

use tree_sitter::Node;

use super::{body, has_keyword, members};
use crate::lint::{Diagnostic, Rule};

pub fn check(class: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if has_keyword(class, text, "abstract")
        || has_keyword(class, text, "static")
        || class.child_by_field_name("superclass").is_some()
    {
        return;
    }
    let Some(body) = body(class) else { return };
    let mut has_instance_member = false;
    let mut has_visible_static_member = false;
    let mut has_constructor = false;
    let mut has_visible_constructor = false;
    for member in members(&body) {
        match member.kind() {
            "method_declaration" | "field_declaration" => {
                if !has_keyword(&member, text, "static") {
                    has_instance_member = true;
                } else if !has_keyword(&member, text, "private") {
                    has_visible_static_member = true;
                }
            }
            "constructor_declaration" => {
                has_constructor = true;
                if !has_keyword(&member, text, "private") && !has_keyword(&member, text, "protected") {
                    has_visible_constructor = true;
                }
            }
            _ => {}
        }
    }
    if !has_instance_member && has_visible_static_member && (!has_constructor || has_visible_constructor) {
        let message = "Utility classes should not have a public or default constructor";
        diagnostics.push(Diagnostic::new(Rule::UtilityClassConstructor, class.start_byte(), message));
    }
}
