//! M006: local variable whose type is obvious from its value (Eclipse save action "Use var"), Java 10:
//! `ArrayList<String> list = new ArrayList<String>()` becomes `var list = new ArrayList<String>()`.
//!
//! Only where `var` infers exactly the declared type, judged from the code alone: a constructor call of the declared
//! type with explicit type arguments or none (not with `<>`, from which `var` would infer `Object` arguments, and not
//! an anonymous class), a cast to the declared type, and a string literal for `String`. Not for fields, several
//! variables in one declaration, array initializers, lambdas, method references, `null` and other literals (`long
//! x = 1` would become `int`). Eclipse also converts other values whose type it knows from type information.

use tree_sitter::Node;

use crate::lint::syntax::{has_comment, named_children, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(declaration: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(declared) = declaration.child_by_field_name("type") else { return };
    let declarators: Vec<Node> =
        named_children(declaration).into_iter().filter(|c| c.kind() == "variable_declarator").collect();
    let [declarator] = declarators.as_slice() else { return };
    // `int a[] = ...` declares an array type in the declarator.
    if declarator.child_by_field_name("dimensions").is_some() || source(&declared, text) == "var" {
        return;
    }
    let Some(value) = declarator.child_by_field_name("value") else { return };
    let declared_type = without_spaces(source(&declared, text));
    let obvious = match value.kind() {
        "object_creation_expression" => {
            let is_anonymous = named_children(&value).iter().any(|c| c.kind() == "class_body");
            let created = value.child_by_field_name("type").map(|t| without_spaces(source(&t, text)));
            // A constructor call with type arguments of its own (`new <T>A()`) has them before the type.
            let has_constructor_type_arguments = value.child_by_field_name("type_arguments").is_some();
            !is_anonymous && !has_constructor_type_arguments && created.is_some_and(|c| c == declared_type)
        }
        "cast_expression" => {
            let types: Vec<Node> = value.children_by_field_name("type", &mut value.walk()).collect();
            matches!(types.as_slice(), [cast] if without_spaces(source(cast, text)) == declared_type)
        }
        "string_literal" => declared_type == "String",
        _ => false,
    };
    if obvious && !declared_type.ends_with("<>") && !has_comment(&declared) {
        let message = "Local variable type can be replaced with 'var'";
        let fix = Edit::new(declared.byte_range(), "var");
        diagnostics.push(Diagnostic::new(Rule::UseVar, declared.start_byte(), message).with_fix(vec![fix]));
    }
}

fn without_spaces(code: &str) -> String {
    code.chars().filter(|c| !c.is_whitespace()).collect()
}
