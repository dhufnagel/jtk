//! N001–N012: naming conventions, with the default formats of the equivalent Checkstyle checks (PMD
//! `*NamingConventions`):
//!
//! | Rule | Names | Checkstyle |
//! |---|---|---|
//! | N001 | classes, interfaces, enums, annotations, records | TypeName |
//! | N002 | methods | MethodName |
//! | N003 | instance fields | MemberName |
//! | N004 | `static final` fields and interface fields | ConstantName |
//! | N005 | other `static` fields | StaticVariableName |
//! | N006 | local variables | LocalVariableName, LocalFinalVariableName |
//! | N007 | method, constructor and lambda parameters | ParameterName, LambdaParameterName |
//! | N008 | `catch` parameters | CatchParameterName |
//! | N009 | pattern variables | PatternVariableName |
//! | N010 | record components | RecordComponentName |
//! | N011 | type parameters | ClassTypeParameterName, MethodTypeParameterName, ... |
//! | N012 | packages | PackageName |
//!
//! N013 (AbbreviationAsWordInName) and N014 (IllegalIdentifierName) are in their own modules.

mod abbreviation_in_name;
mod illegal_identifier;

use tree_sitter::Node;

use super::syntax::{has_modifier, modifiers, named_children, source};
use super::{Check, ConfiguredCheck, Diagnostic, Rule};

/// The formats, as Checkstyle's regular expressions spell them.
#[derive(Clone, Copy)]
enum Format {
    /// `^[A-Z][a-zA-Z0-9]*$`
    UpperCamel,
    /// `^[a-z][a-zA-Z0-9]*$`
    LowerCamel,
    /// `^([a-z][a-zA-Z0-9]*|_)$`
    LowerCamelOrUnnamed,
    /// `^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$`
    Constant,
    /// `^(e|t|ex|[a-z][a-z][a-zA-Z]+|_)$`
    Catch,
    /// `^[A-Z]$`
    SingleUpper,
    /// `^[a-z]+(\.[a-zA-Z_]\w*)*$`
    Package,
}

impl Format {
    fn pattern(self) -> &'static str {
        match self {
            Format::UpperCamel => "^[A-Z][a-zA-Z0-9]*$",
            Format::LowerCamel => "^[a-z][a-zA-Z0-9]*$",
            Format::LowerCamelOrUnnamed => "^([a-z][a-zA-Z0-9]*|_)$",
            Format::Constant => "^[A-Z][A-Z0-9]*(_[A-Z0-9]+)*$",
            Format::Catch => "^(e|t|ex|[a-z][a-z][a-zA-Z]+|_)$",
            Format::SingleUpper => "^[A-Z]$",
            Format::Package => "^[a-z]+(\\.[a-zA-Z_]\\w*)*$",
        }
    }

    fn matches(self, name: &str) -> bool {
        let alphanumeric = |rest: &str| rest.chars().all(|c| c.is_ascii_alphanumeric());
        let mut chars = name.chars();
        let first = chars.next();
        let rest = chars.as_str();
        match self {
            Format::UpperCamel => first.is_some_and(|c| c.is_ascii_uppercase()) && alphanumeric(rest),
            Format::LowerCamel => first.is_some_and(|c| c.is_ascii_lowercase()) && alphanumeric(rest),
            Format::LowerCamelOrUnnamed => name == "_" || Format::LowerCamel.matches(name),
            Format::Constant => {
                first.is_some_and(|c| c.is_ascii_uppercase())
                    && name.split('_').all(|part| {
                        !part.is_empty() && part.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                    })
            }
            Format::Catch => {
                matches!(name, "e" | "t" | "ex" | "_")
                    || (name.len() >= 3
                        && name.chars().take(2).all(|c| c.is_ascii_lowercase())
                        && name.chars().skip(2).all(|c| c.is_ascii_alphabetic()))
            }
            Format::SingleUpper => name.len() == 1 && first.is_some_and(|c| c.is_ascii_uppercase()),
            Format::Package => {
                let mut segments = name.split('.');
                let is_word = |s: &str| {
                    s.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                };
                segments.next().is_some_and(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase()))
                    && segments.all(is_word)
            }
        }
    }
}

fn report(rule: Rule, name: Option<Node>, format: Format, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(name) = name else { return };
    let value = source(&name, text);
    if !format.matches(value) {
        let message = format!("Name '{value}' must match pattern '{}'", format.pattern());
        diagnostics.push(Diagnostic::new(rule, name.start_byte(), message));
    }
}

/// The name of a parameter, also of a varargs one (`String... values`).
fn parameter_name_node<'t>(parameter: &Node<'t>) -> Option<Node<'t>> {
    match parameter.kind() {
        "spread_parameter" => named_children(parameter)
            .into_iter()
            .find(|c| c.kind() == "variable_declarator")
            .and_then(|d| d.child_by_field_name("name")),
        _ => parameter.child_by_field_name("name"),
    }
}

fn declarator_names<'t>(declaration: &Node<'t>) -> Vec<Node<'t>> {
    named_children(declaration)
        .into_iter()
        .filter(|d| d.kind() == "variable_declarator")
        .filter_map(|d| d.child_by_field_name("name"))
        .collect()
}

pub fn type_name(declaration: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    report(Rule::TypeName, declaration.child_by_field_name("name"), Format::UpperCamel, text, diagnostics);
}

/// Overriding methods are skipped: their name is not theirs to choose.
pub fn method_name(method: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let is_override = modifiers(method).is_some_and(|m| {
        named_children(&m)
            .iter()
            .any(|a| a.child_by_field_name("name").is_some_and(|n| source(&n, text).ends_with("Override")))
    });
    if !is_override {
        report(Rule::MethodName, method.child_by_field_name("name"), Format::LowerCamel, text, diagnostics);
    }
}

/// The kind of a field: instance, constant (`static final` or in an interface) or static.
fn field_rule(field: &Node, text: &str) -> Rule {
    let is_static = has_modifier(field, text, "static");
    let is_final = has_modifier(field, text, "final");
    match (field.kind(), is_static, is_final) {
        ("constant_declaration", _, _) | (_, true, true) => Rule::ConstantName,
        (_, true, false) => Rule::StaticVariableName,
        _ => Rule::MemberName,
    }
}

pub fn member_name(field: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if field_rule(field, text) == Rule::MemberName {
        for name in declarator_names(field) {
            report(Rule::MemberName, Some(name), Format::LowerCamel, text, diagnostics);
        }
    }
}

pub fn constant_name(field: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if field_rule(field, text) == Rule::ConstantName {
        let names = declarator_names(field)
            .into_iter()
            .filter(|n| !matches!(source(n, text), "serialVersionUID" | "serialPersistentFields"));
        for name in names {
            report(Rule::ConstantName, Some(name), Format::Constant, text, diagnostics);
        }
    }
}

pub fn static_variable_name(field: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if field_rule(field, text) == Rule::StaticVariableName {
        for name in declarator_names(field) {
            report(Rule::StaticVariableName, Some(name), Format::LowerCamel, text, diagnostics);
        }
    }
}

pub fn local_variable_name(node: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let names = match node.kind() {
        "local_variable_declaration" => declarator_names(node),
        _ => node.child_by_field_name("name").into_iter().collect(),
    };
    for name in names {
        report(Rule::LocalVariableName, Some(name), Format::LowerCamelOrUnnamed, text, diagnostics);
    }
}

pub fn parameter_name(parameter: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let owner = parameter.parent().and_then(|parameters| parameters.parent());
    let name = parameter_name_node(parameter);
    let format = match owner.map(|o| o.kind()) {
        Some("lambda_expression") => Format::LowerCamelOrUnnamed,
        Some("method_declaration" | "constructor_declaration") => Format::LowerCamel,
        _ => return,
    };
    report(Rule::ParameterName, name, format, text, diagnostics);
}

/// Lambda parameters without types: `x -> ...` or `(x, y) -> ...`.
pub fn lambda_parameter_name(lambda: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(parameters) = lambda.child_by_field_name("parameters") else { return };
    let names = match parameters.kind() {
        "identifier" => vec![parameters],
        "inferred_parameters" => named_children(&parameters),
        _ => return,
    };
    for name in names {
        report(Rule::ParameterName, Some(name), Format::LowerCamelOrUnnamed, text, diagnostics);
    }
}

pub fn catch_parameter_name(parameter: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    report(Rule::CatchParameterName, parameter.child_by_field_name("name"), Format::Catch, text, diagnostics);
}

pub fn pattern_variable_name(node: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let name = match node.kind() {
        "instanceof_expression" => node.child_by_field_name("name"),
        _ => named_children(node).into_iter().last().filter(|n| n.kind() == "identifier"),
    };
    report(Rule::PatternVariableName, name, Format::LowerCamelOrUnnamed, text, diagnostics);
}

pub fn record_component_name(record: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(components) = record.child_by_field_name("parameters") else { return };
    for component in named_children(&components) {
        report(Rule::RecordComponentName, parameter_name_node(&component), Format::LowerCamel, text, diagnostics);
    }
}

pub fn type_parameter_name(parameter: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let name = named_children(parameter).into_iter().find(|c| matches!(c.kind(), "type_identifier" | "identifier"));
    report(Rule::TypeParameterName, name, Format::SingleUpper, text, diagnostics);
}

/// The package name without the whitespace and comments it may be written with.
pub fn package_name(declaration: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(name) =
        named_children(declaration).into_iter().find(|c| matches!(c.kind(), "scoped_identifier" | "identifier"))
    else {
        return;
    };
    let mut segments = Vec::new();
    super::syntax::for_each_descendant(&name, &[], |n| {
        if n.kind() == "identifier" {
            segments.push(source(n, text));
        }
    });
    let package = if segments.is_empty() { source(&name, text).to_string() } else { segments.join(".") };
    if !Format::Package.matches(&package) {
        let message = format!("Name '{package}' must match pattern '{}'", Format::Package.pattern());
        diagnostics.push(Diagnostic::new(Rule::PackageName, name.start_byte(), message));
    }
}

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[crate::ast::Tag], super::tree::TreeCheck)] = &[];

pub(super) const CHECKS: &[(Rule, &[&str], Check)] = &[
    (
        Rule::TypeName,
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "annotation_type_declaration",
            "record_declaration",
        ],
        type_name,
    ),
    (Rule::MethodName, &["method_declaration"], method_name),
    (Rule::MemberName, &["field_declaration"], member_name),
    (Rule::ConstantName, &["field_declaration", "constant_declaration"], constant_name),
    (Rule::StaticVariableName, &["field_declaration"], static_variable_name),
    (
        Rule::LocalVariableName,
        &["local_variable_declaration", "enhanced_for_statement", "resource"],
        local_variable_name,
    ),
    (Rule::ParameterName, &["formal_parameter", "spread_parameter"], parameter_name),
    (Rule::ParameterName, &["lambda_expression"], lambda_parameter_name),
    (Rule::CatchParameterName, &["catch_formal_parameter"], catch_parameter_name),
    (
        Rule::PatternVariableName,
        &["instanceof_expression", "type_pattern", "record_pattern_component"],
        pattern_variable_name,
    ),
    (Rule::RecordComponentName, &["record_declaration"], record_component_name),
    (Rule::TypeParameterName, &["type_parameter"], type_parameter_name),
    (Rule::PackageName, &["package_declaration"], package_name),
    (
        Rule::IllegalIdentifier,
        &[
            "class_declaration",
            "interface_declaration",
            "enum_declaration",
            "annotation_type_declaration",
            "record_declaration",
            "annotation_type_element_declaration",
            "method_declaration",
            "enum_constant",
            "field_declaration",
            "constant_declaration",
            "local_variable_declaration",
            "enhanced_for_statement",
            "formal_parameter",
            "spread_parameter",
            "catch_formal_parameter",
            "inferred_parameters",
            "lambda_expression",
            "instanceof_expression",
            "type_pattern",
            "record_pattern_component",
        ],
        illegal_identifier::check,
    ),
];

/// The checks of rules with parameters.
pub(super) const CONFIGURED_CHECKS: &[(Rule, &[&str], ConfiguredCheck)] = &[(
    Rule::AbbreviationInName,
    &[
        "class_declaration",
        "interface_declaration",
        "enum_declaration",
        "annotation_type_declaration",
        "record_declaration",
        "annotation_type_element_declaration",
        "method_declaration",
        "field_declaration",
        "local_variable_declaration",
        "enhanced_for_statement",
        "formal_parameter",
        "spread_parameter",
        "catch_formal_parameter",
        "inferred_parameters",
        "instanceof_expression",
        "type_pattern",
        "record_pattern_component",
    ],
    abbreviation_in_name::check,
)];
