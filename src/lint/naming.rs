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

mod method_named_like_class;

use super::tree::{Context, TreeCheck};
use super::{Diagnostic, Rule};
use jtk_parser::ast::{ClassKind, K, Modifier, Mods, Tag, Tree, VarDecl};

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

fn report(rule: Rule, name: &str, offset: usize, format: Format, diagnostics: &mut Vec<Diagnostic>) {
    if !format.matches(name) {
        let message = format!("Name '{name}' must match pattern '{}'", format.pattern());
        diagnostics.push(Diagnostic::new(rule, offset, message));
    }
}

fn report_variable(rule: Rule, variable: &VarDecl, format: Format, diagnostics: &mut Vec<Diagnostic>) {
    report(rule, &variable.name, variable.name_pos, format, diagnostics);
}

/// Whether a variable declaration (`K::Vars`) declares fields: its parent is a class body.
fn is_field(context: &Context) -> bool {
    matches!(context.parent().map(|p| &p.kind), Some(K::Class(_) | K::NewClass { .. }))
}

/// Whether the parent is the body of an interface or annotation type, whose fields are constants.
fn is_interface_member(context: &Context) -> bool {
    matches!(context.parent().map(|p| &p.kind),
        Some(K::Class(c)) if matches!(c.kind, ClassKind::Interface | ClassKind::Annotation))
}

/// Whether a method is an element of an annotation type, not a method.
fn is_annotation_element(context: &Context) -> bool {
    matches!(context.parent().map(|p| &p.kind), Some(K::Class(c)) if c.kind == ClassKind::Annotation)
}

/// Whether the declaration has a `static` or a `final` modifier.
fn is_static_or_final(mods: &Mods) -> bool {
    mods.has(Modifier::Static) || mods.has(Modifier::Final)
}

/// The rule for the names of fields: constants (`static final` or in an interface or annotation type), static or
/// instance fields.
fn field_rule(vars: &[VarDecl], context: &Context) -> Rule {
    if is_interface_member(context) {
        return Rule::ConstantName;
    }
    let Some(first) = vars.first() else { return Rule::MemberName };
    match (first.mods.has(Modifier::Static), first.mods.has(Modifier::Final)) {
        (true, true) => Rule::ConstantName,
        (true, false) => Rule::StaticVariableName,
        _ => Rule::MemberName,
    }
}

pub fn type_name(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if let K::Class(class) = &tree.kind {
        report(Rule::TypeName, &class.name, class.name_pos, Format::UpperCamel, diagnostics);
    }
}

/// Overriding methods are skipped: their name is not theirs to choose.
pub fn method_name(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    if method.ret.is_none() || is_annotation_element(context) {
        return;
    }
    let is_override = method.mods.annotations.iter().any(|a| match &a.kind {
        K::Annotation { ty, .. } => context.source(ty).ends_with("Override"),
        _ => false,
    });
    if !is_override {
        report(Rule::MethodName, &method.name, method.name_pos, Format::LowerCamel, diagnostics);
    }
}

/// The fields of a declaration if their names are checked by `rule`.
fn fields_of<'t>(tree: &'t Tree, context: &Context, rule: Rule) -> &'t [VarDecl] {
    match &tree.kind {
        K::Vars(vars) if is_field(context) && field_rule(vars, context) == rule => vars,
        _ => &[],
    }
}

pub fn member_name(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    for field in fields_of(tree, context, Rule::MemberName) {
        report_variable(Rule::MemberName, field, Format::LowerCamel, diagnostics);
    }
}

pub fn constant_name(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let fields = fields_of(tree, context, Rule::ConstantName);
    for field in fields.iter().filter(|f| !matches!(f.name.as_str(), "serialVersionUID" | "serialPersistentFields")) {
        report_variable(Rule::ConstantName, field, Format::Constant, diagnostics);
    }
}

pub fn static_variable_name(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    for field in fields_of(tree, context, Rule::StaticVariableName) {
        report_variable(Rule::StaticVariableName, field, Format::LowerCamel, diagnostics);
    }
}

/// Local variables, also of enhanced `for` statements and resources.
pub fn local_variable_name(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let mut report_local =
        |v: &VarDecl| report_variable(Rule::LocalVariableName, v, Format::LowerCamelOrUnnamed, diagnostics);
    match &tree.kind {
        K::Vars(vars) if !is_field(context) => vars.iter().for_each(report_local),
        K::ForEach { var, .. } => report_local(var),
        K::Try { resources, .. } => {
            for resource in resources {
                if let K::Var(var) = &resource.kind {
                    report_local(var);
                }
            }
        }
        _ => {}
    }
}

/// Parameters of methods, constructors and lambdas.
pub fn parameter_name(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let (params, format) = match &tree.kind {
        K::Method(method) => (&method.params, Format::LowerCamel),
        K::Lambda { params, .. } => (params, Format::LowerCamelOrUnnamed),
        _ => return,
    };
    for param in params {
        report_variable(Rule::ParameterName, param, format, diagnostics);
    }
}

pub fn catch_parameter_name(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if let K::Try { catches, .. } = &tree.kind {
        for catch in catches {
            report_variable(Rule::CatchParameterName, &catch.param, Format::Catch, diagnostics);
        }
    }
}

pub fn pattern_variable_name(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if let K::BindingPattern { var } = &tree.kind {
        report_variable(Rule::PatternVariableName, var, Format::LowerCamelOrUnnamed, diagnostics);
    }
}

pub fn record_component_name(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    if let K::Class(class) = &tree.kind {
        for component in &class.record_components {
            report_variable(Rule::RecordComponentName, component, Format::LowerCamel, diagnostics);
        }
    }
}

pub fn type_parameter_name(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::TypeParameter { annotations, name, .. } = &tree.kind else { return };
    let offset = match annotations.last() {
        Some(annotation) => context.tokens().next(annotation.end).map_or(tree.start, |t| t.start),
        None => tree.start,
    };
    report(Rule::TypeParameterName, name, offset, Format::SingleUpper, diagnostics);
}

/// The package name without the whitespace and comments it may be written with.
pub fn package_name(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::CompilationUnit { package: Some((_, name)), .. } = &tree.kind else { return };
    let mut package = String::new();
    append_qualified_name(name, &mut package);
    report(Rule::PackageName, &package, name.start, Format::Package, diagnostics);
}

/// Appends the identifiers of a qualified name (`MemberSelect`s of an `Ident`), separated by `.`.
fn append_qualified_name(name: &Tree, out: &mut String) {
    match &name.kind {
        K::Ident(identifier) => out.push_str(identifier),
        K::MemberSelect { expr, name } => {
            append_qualified_name(expr, out);
            out.push('.');
            out.push_str(name);
        }
        _ => {}
    }
}

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], TreeCheck)] = &[
    (Rule::TypeName, &[Tag::Class], type_name),
    (Rule::MethodName, &[Tag::Method], method_name),
    (Rule::MemberName, &[Tag::Vars], member_name),
    (Rule::ConstantName, &[Tag::Vars], constant_name),
    (Rule::StaticVariableName, &[Tag::Vars], static_variable_name),
    (Rule::LocalVariableName, &[Tag::Vars, Tag::ForEach, Tag::Try], local_variable_name),
    (Rule::ParameterName, &[Tag::Method, Tag::Lambda], parameter_name),
    (Rule::CatchParameterName, &[Tag::Try], catch_parameter_name),
    (Rule::PatternVariableName, &[Tag::BindingPattern], pattern_variable_name),
    (Rule::RecordComponentName, &[Tag::Class], record_component_name),
    (Rule::TypeParameterName, &[Tag::TypeParameter], type_parameter_name),
    (Rule::PackageName, &[Tag::CompilationUnit], package_name),
    (Rule::IllegalIdentifier, DECLARING, illegal_identifier::check),
    (Rule::AbbreviationInName, DECLARING, abbreviation_in_name::check),
    (Rule::MethodNamedLikeClass, &[Tag::Method], method_named_like_class::check),
];

/// The kinds of trees that declare the names N013 and N014 check.
const DECLARING: &[Tag] =
    &[Tag::Class, Tag::Method, Tag::Vars, Tag::ForEach, Tag::Lambda, Tag::Try, Tag::BindingPattern];
