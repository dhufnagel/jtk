//! S008: modifiers that the language implies (Checkstyle RedundantModifier, PMD UnnecessaryModifier), following
//! Checkstyle's cases:
//!
//! - interface and annotation members: `public`; `static`, except on methods; `abstract` and `final`, except on
//!   classes; `static` and `abstract` on interfaces and annotations themselves
//! - `static` on enums, `final` and `static` on records
//! - `final` on methods that cannot be overridden: private, in final, anonymous or enum constant classes, and static
//!   enum methods (unless `@SafeVarargs`, which requires it); on parameters of methods without a body; on try
//!   resources and on unnamed variables (`_`)
//! - enum constructor modifiers, and `public` on constructors of classes that are neither public nor protected
//! - `strictfp`, which has no effect since Java 17

use tree_sitter::Node;

use crate::lint::syntax::{has_modifier, modifier_keywords, modifiers, named_children, removal_range, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(declaration: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let keywords = modifier_keywords(declaration);
    if keywords.is_empty() {
        return;
    }
    let kind = declaration.kind();
    let parent_kind = declaration.parent().map(|p| p.kind());
    let is_interface_member = matches!(parent_kind, Some("interface_body" | "annotation_type_body"));
    let mut redundant: Vec<&str> = Vec::new();
    match kind {
        "interface_declaration" | "annotation_type_declaration" => redundant.extend(["static", "abstract"]),
        "enum_declaration" => redundant.push("static"),
        "record_declaration" => redundant.extend(["final", "static"]),
        "method_declaration" if is_final_redundant(declaration, text) => redundant.push("final"),
        "resource" => redundant.push("final"),
        "local_variable_declaration" | "catch_formal_parameter" | "formal_parameter"
            if declares_unnamed(declaration, text) =>
        {
            redundant.push("final")
        }
        "constructor_declaration" => {
            if parent_kind == Some("enum_body_declarations") {
                // Enum constructors are private; any modifier is redundant.
                redundant.extend(keywords.iter().map(|k| source(k, text)));
            } else if !declaration
                .parent()
                .and_then(|b| b.parent())
                .is_some_and(|c| is_public_class(&c, text) || has_modifier(&c, text, "protected"))
            {
                redundant.push("public");
            }
        }
        _ => {}
    }
    if is_interface_member {
        redundant.push("public");
        if kind != "method_declaration" {
            redundant.push("static");
        }
        if kind != "class_declaration" {
            redundant.extend(["abstract", "final"]);
        }
    }
    redundant.push("strictfp");
    for keyword in keywords.iter().filter(|k| redundant.contains(&source(k, text))) {
        let message = format!("Redundant '{}' modifier", source(keyword, text));
        let fix = Edit::new(removal_range(keyword, text), "");
        diagnostics.push(Diagnostic::new(Rule::RedundantModifier, keyword.start_byte(), message).with_fix(vec![fix]));
    }
    if kind == "method_declaration" && declaration.child_by_field_name("body").is_none() {
        check_abstract_parameters(declaration, text, diagnostics);
    }
}

/// `final` on parameters of a method without body has no effect.
fn check_abstract_parameters(method: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(parameters) = method.child_by_field_name("parameters") else { return };
    for parameter in named_children(&parameters) {
        for keyword in modifier_keywords(&parameter).iter().filter(|k| source(k, text) == "final") {
            let fix = Edit::new(removal_range(keyword, text), "");
            let diagnostic =
                Diagnostic::new(Rule::RedundantModifier, keyword.start_byte(), "Redundant 'final' modifier");
            diagnostics.push(diagnostic.with_fix(vec![fix]));
        }
    }
}

/// Whether a method cannot be overridden even without `final`. `@SafeVarargs` needs `final` (or `static` or
/// `private`) on the method, so `final` is kept there.
fn is_final_redundant(method: &Node, text: &str) -> bool {
    if has_annotation(method, text, "SafeVarargs") {
        return false;
    }
    if has_modifier(method, text, "private") {
        return true;
    }
    let Some(body) = method.parent() else { return false };
    match (body.kind(), body.parent()) {
        // Enum methods can be overridden in the bodies of the constants, unless static.
        ("enum_body_declarations", _) => has_modifier(method, text, "static"),
        // The bodies of enum constants and anonymous classes cannot be extended.
        ("class_body", Some(owner)) if matches!(owner.kind(), "enum_constant" | "object_creation_expression") => true,
        ("class_body", Some(owner)) => owner.kind() == "class_declaration" && has_modifier(&owner, text, "final"),
        _ => false,
    }
}

/// Whether the declaration is annotated with the annotation, by simple or qualified name.
fn has_annotation(declaration: &Node, text: &str, name: &str) -> bool {
    let Some(modifiers) = modifiers(declaration) else { return false };
    named_children(&modifiers).iter().filter(|m| matches!(m.kind(), "marker_annotation" | "annotation")).any(|a| {
        a.child_by_field_name("name").is_some_and(|n| {
            let written = source(&n, text);
            written == name || written.strip_suffix(name).is_some_and(|q| q.ends_with('.'))
        })
    })
}

fn declares_unnamed(declaration: &Node, text: &str) -> bool {
    let name_is_unnamed = |n: Node| n.child_by_field_name("name").is_some_and(|name| source(&name, text) == "_");
    match declaration.kind() {
        "local_variable_declaration" => {
            named_children(declaration).into_iter().filter(|d| d.kind() == "variable_declarator").any(name_is_unnamed)
        }
        _ => name_is_unnamed(*declaration),
    }
}

/// Whether a type is accessible everywhere: public (explicitly, or implicitly as a member of an interface), and
/// nested only in types that are accessible everywhere.
fn is_public_class(class: &Node, text: &str) -> bool {
    let Some(container) = class.parent() else { return false };
    let is_public =
        has_modifier(class, text, "public") || matches!(container.kind(), "interface_body" | "annotation_type_body");
    if !is_public {
        return false;
    }
    let enclosing = match container.kind() {
        "program" => return true,
        "class_body" | "interface_body" | "annotation_type_body" | "enum_body" => container.parent(),
        "enum_body_declarations" => container.parent().and_then(|b| b.parent()),
        // Local classes.
        _ => None,
    };
    // Members of anonymous classes and enum constants are not accessible outside them.
    enclosing.is_some_and(|e| e.kind().ends_with("_declaration") && is_public_class(&e, text))
}
