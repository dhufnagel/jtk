//! S012: fields explicitly initialized to their type's default value (Checkstyle ExplicitInitialization, PMD
//! RedundantFieldInitializer): `null`, `false`, zero and `'\0'`. Final fields, interface fields and local variables
//! are not checked.
//!
//! The fix removes the initializer. Strictly, an instance field initializer runs after the superclass constructor,
//! so it would reset a value that an overridden method called from that constructor assigned; like the IDEs' quick
//! fixes, the fix assumes that code does not depend on this.

use tree_sitter::Node;

use super::contains_comment;
use crate::lint::syntax::{has_modifier, named_children, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(field: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if has_modifier(field, text, "final") {
        return;
    }
    let Some(field_type) = field.child_by_field_name("type") else { return };
    let declarators = named_children(field).into_iter().filter(|d| d.kind() == "variable_declarator");
    for declarator in declarators {
        let (Some(name), Some(value)) =
            (declarator.child_by_field_name("name"), declarator.child_by_field_name("value"))
        else {
            continue;
        };
        let is_array = declarator.child_by_field_name("dimensions").is_some();
        let Some(default) = default_value(&field_type, is_array, &value, text) else { continue };
        let message =
            format!("Variable '{}' explicitly initialized to '{default}' (its default value)", source(&name, text));
        let mut diagnostic = Diagnostic::new(Rule::ExplicitInitialization, name.start_byte(), message);
        let before_initializer = declarator.child_by_field_name("dimensions").unwrap_or(name).end_byte();
        if !contains_comment(&declarator) {
            diagnostic = diagnostic.with_fix(vec![Edit::new(before_initializer..value.end_byte(), "")]);
        }
        diagnostics.push(diagnostic);
    }
}

/// The default value that `value` explicitly initializes a field of the type to, if it does.
fn default_value(field_type: &Node, is_array: bool, value: &Node, text: &str) -> Option<&'static str> {
    if value.kind() == "null_literal" {
        return Some("null");
    }
    if is_array {
        return None;
    }
    match source(field_type, text) {
        "boolean" if value.kind() == "false" => Some("false"),
        "byte" | "short" | "int" | "long" | "float" | "double" if is_zero(value, text) => Some("0"),
        "char" if is_zero(value, text) || source(value, text) == "'\\0'" => Some("\\0"),
        _ => None,
    }
}

/// Whether the node is a numeric literal with the value zero.
fn is_zero(literal: &Node, text: &str) -> bool {
    let code = source(literal, text).replace('_', "").to_ascii_lowercase();
    match literal.kind() {
        "decimal_integer_literal" | "octal_integer_literal" => code.trim_end_matches('l').chars().all(|c| c == '0'),
        "hex_integer_literal" | "binary_integer_literal" => code[2..].trim_end_matches('l').chars().all(|c| c == '0'),
        "decimal_floating_point_literal" => code.trim_end_matches(['f', 'd']).parse::<f64>().is_ok_and(|v| v == 0.0),
        "hex_floating_point_literal" => {
            code[2..].split('p').next().is_some_and(|mantissa| mantissa.chars().all(|c| matches!(c, '0' | '.')))
        }
        _ => false,
    }
}
