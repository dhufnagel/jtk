//! S012: fields explicitly initialized to their type's default value (Checkstyle ExplicitInitialization, PMD
//! RedundantFieldInitializer): `null`, `false`, zero and `'\0'`. Final fields, interface fields and local variables
//! are not checked.
//!
//! The fix removes the initializer. Strictly, an instance field initializer runs after the superclass constructor,
//! so it would reset a value that an overridden method called from that constructor assigned; like the IDEs' quick
//! fixes, the fix assumes that code does not depend on this.

use crate::lint::tree::{Context, has_modifier};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{ClassKind, K, LitKind, Tree};

pub fn check(field: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Vars(declarators) = &field.kind else { return };
    let is_field = context.parent().is_some_and(|p| match &p.kind {
        K::Class(class) => !matches!(class.kind, ClassKind::Interface | ClassKind::Annotation),
        K::NewClass { .. } => true,
        _ => false,
    });
    if !is_field || has_modifier(&declarators[0].mods, context.text, context.tokens(), "final") {
        return;
    }
    for declarator in declarators {
        let (Some(field_type), Some(value)) = (declarator.ty.as_deref(), declarator.init.as_deref()) else { continue };
        let name_end = declarator.name_pos + declarator.name.len();
        // C-style dimensions (`int a[]`) extend the type past the name.
        let is_array = field_type.end > declarator.name_pos;
        let Some(default) = default_value(field_type, is_array, value, context) else { continue };
        let message =
            format!("Variable '{}' explicitly initialized to '{default}' (its default value)", declarator.name);
        let mut diagnostic = Diagnostic::new(Rule::ExplicitInitialization, declarator.name_pos, message);
        if !context.tokens().has_comment(declarator.name_pos..declarator.end) {
            let before_initializer = if is_array { field_type.end } else { name_end };
            diagnostic = diagnostic.with_fix(vec![Edit::new(before_initializer..value.end, "")]);
        }
        diagnostics.push(diagnostic);
    }
}

/// The default value that `value` explicitly initializes a field of the type to, if it does.
fn default_value(field_type: &Tree, is_array: bool, value: &Tree, context: &Context) -> Option<&'static str> {
    let K::Literal(kind) = value.kind else { return None };
    if kind == LitKind::Null {
        return Some("null");
    }
    if is_array {
        return None;
    }
    let code = context.source(value);
    match context.source(field_type) {
        "boolean" if code == "false" => Some("false"),
        "byte" | "short" | "int" | "long" | "float" | "double" if is_zero(kind, code) => Some("0"),
        "char" if is_zero(kind, code) || code == "'\\0'" => Some("\\0"),
        _ => None,
    }
}

/// Whether the literal is a number with the value zero (`-0` is an operation in tree-sitter's syntax tree, whose
/// behaviour the rule keeps).
fn is_zero(kind: LitKind, code: &str) -> bool {
    if code.starts_with('-') {
        return false;
    }
    let code = code.replace('_', "").to_ascii_lowercase();
    let based = code.strip_prefix("0x").or_else(|| code.strip_prefix("0b"));
    match kind {
        LitKind::Int | LitKind::Long => based.unwrap_or(&code).trim_end_matches('l').chars().all(|c| c == '0'),
        LitKind::Float | LitKind::Double => match code.strip_prefix("0x") {
            Some(hex) => hex.split('p').next().is_some_and(|mantissa| mantissa.chars().all(|c| matches!(c, '0' | '.'))),
            None => code.trim_end_matches(['f', 'd']).parse::<f64>().is_ok_and(|v| v == 0.0),
        },
        _ => false,
    }
}
