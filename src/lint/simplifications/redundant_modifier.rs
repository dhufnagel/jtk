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

use crate::ast::{ClassDecl, ClassKind, K, Mods, Tree, VarDecl};
use crate::lint::tree::{Context, has_modifier, modifier_keywords};
use crate::lint::{Diagnostic, Edit, Rule};

use super::removal_range;

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let parent = context.parent();
    // Declarations in an interface or annotation are its members.
    let is_interface_member = parent.is_some_and(
        |p| matches!(&p.kind, K::Class(c) if matches!(c.kind, ClassKind::Interface | ClassKind::Annotation)),
    );
    match &tree.kind {
        K::Class(class) => {
            let mut redundant = match class.kind {
                ClassKind::Interface | ClassKind::Annotation => vec!["static", "abstract"],
                ClassKind::Enum => vec!["static"],
                ClassKind::Record => vec!["final", "static"],
                ClassKind::Class => vec![],
            };
            if is_interface_member {
                redundant.extend(["public", "static"]);
                if class.kind != ClassKind::Class {
                    redundant.extend(["abstract", "final"]);
                }
            }
            report(&class.mods, &redundant, context, diagnostics);
            for component in &class.record_components {
                check_parameter(component, false, context, diagnostics);
            }
        }
        K::Method(method) => {
            let mut redundant = Vec::new();
            let is_annotation_element =
                parent.is_some_and(|p| matches!(&p.kind, K::Class(c) if c.kind == ClassKind::Annotation));
            match &method.ret {
                _ if method.compact_ctor => {}
                None => constructor_redundant(tree, &method.mods, context, &mut redundant),
                Some(_) if !is_annotation_element && is_final_redundant(tree, &method.mods, context) => {
                    redundant.push("final")
                }
                Some(_) => {}
            }
            if is_interface_member {
                redundant.push("public");
                if is_annotation_element {
                    redundant.push("static");
                }
                redundant.extend(["abstract", "final"]);
            }
            if !method.compact_ctor {
                report(&method.mods, &redundant, context, diagnostics);
            }
            let without_body = method.ret.is_some() && !is_annotation_element && method.body.is_none();
            for parameter in &method.params {
                check_parameter(parameter, without_body, context, diagnostics);
            }
        }
        K::Vars(vars) => {
            let mut redundant = Vec::new();
            if vars.iter().any(|v| v.name == "_") {
                redundant.push("final");
            }
            if is_interface_member {
                redundant.extend(["public", "static", "abstract", "final"]);
            }
            report(&vars[0].mods, &redundant, context, diagnostics);
        }
        K::Try { resources, catches, .. } => {
            for resource in resources {
                if let K::Var(var) = &resource.kind {
                    report(&var.mods, &["final"], context, diagnostics);
                }
            }
            for catch in catches {
                let redundant: &[&str] = if catch.param.name == "_" { &["final"] } else { &[] };
                report(&catch.param.mods, redundant, context, diagnostics);
            }
        }
        K::Lambda { params, .. } => {
            for parameter in params.iter().filter(|p| p.ty.is_some()) {
                check_parameter(parameter, false, context, diagnostics);
            }
        }
        _ => {}
    }
}

/// The modifiers of a formal parameter, and in a method without body, its `final` (reported twice on an unnamed
/// parameter there, like before).
fn check_parameter(
    parameter: &VarDecl,
    in_method_without_body: bool,
    context: &Context,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let is_variable_arity = context.tokens().previous(parameter.name_pos).is_some_and(|t| &context.text[t] == "...");
    if !is_variable_arity {
        let redundant: &[&str] = if parameter.name == "_" { &["final"] } else { &[] };
        report(&parameter.mods, redundant, context, diagnostics);
    }
    if in_method_without_body {
        for (_, keyword) in modifier_keywords(&parameter.mods, context.text, context.tokens())
            .into_iter()
            .filter(|(k, _)| *k == "final")
        {
            let fix = Edit::new(removal_range(keyword.clone(), context.text), "");
            let diagnostic = Diagnostic::new(Rule::RedundantModifier, keyword.start, "Redundant 'final' modifier");
            diagnostics.push(diagnostic.with_fix(vec![fix]));
        }
    }
}

/// Reports the modifier keywords that are `redundant`, and `strictfp`.
fn report(mods: &Mods, redundant: &[&str], context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    for (keyword, range) in modifier_keywords(mods, context.text, context.tokens()) {
        if keyword != "strictfp" && !redundant.contains(&keyword) {
            continue;
        }
        let message = format!("Redundant '{keyword}' modifier");
        let fix = Edit::new(removal_range(range.clone(), context.text), "");
        diagnostics.push(Diagnostic::new(Rule::RedundantModifier, range.start, message).with_fix(vec![fix]));
    }
}

/// The redundant modifiers of a constructor: all in an enum, where constructors are private, and `public` in a class
/// that is neither public nor protected.
fn constructor_redundant<'a>(constructor: &Tree, mods: &Mods, context: &'a Context, redundant: &mut Vec<&'a str>) {
    let (text, tokens) = (context.text, context.tokens());
    let owner = context.parent().and_then(|p| match &p.kind {
        K::Class(class) if is_member(class, constructor) => Some(class),
        _ => None,
    });
    if owner.is_some_and(|class| class.kind == ClassKind::Enum) {
        redundant.extend(modifier_keywords(mods, text, tokens).into_iter().map(|(k, _)| k));
        return;
    }
    let accessible = owner.is_some_and(|class| {
        is_public_class(&context.ancestors, context.ancestors.len() - 1, context)
            || has_modifier(&class.mods, text, tokens, "protected")
    });
    if !accessible {
        redundant.push("public");
    }
}

/// Whether a method cannot be overridden even without `final`. `@SafeVarargs` needs `final` (or `static` or
/// `private`) on the method, so `final` is kept there.
fn is_final_redundant(method: &Tree, mods: &Mods, context: &Context) -> bool {
    let (text, tokens) = (context.text, context.tokens());
    if has_annotation(mods, "SafeVarargs", context) {
        return false;
    }
    if has_modifier(mods, text, tokens, "private") {
        return true;
    }
    match context.parent().map(|p| &p.kind) {
        Some(K::Class(class)) if class.kind == ClassKind::Enum => {
            // Enum methods can be overridden in the bodies of the constants, unless static; the bodies of the
            // constants cannot be extended.
            !is_member(class, method) || has_modifier(mods, text, tokens, "static")
        }
        Some(K::Class(class)) => class.kind == ClassKind::Class && has_modifier(&class.mods, text, tokens, "final"),
        // Anonymous classes cannot be extended.
        Some(K::NewClass { .. }) => true,
        _ => false,
    }
}

/// Whether the declaration is annotated with the annotation, by simple or qualified name.
fn has_annotation(mods: &Mods, name: &str, context: &Context) -> bool {
    mods.annotations.iter().any(|a| {
        let K::Annotation { ty, .. } = &a.kind else { return false };
        let written = context.source(ty);
        written == name || written.strip_suffix(name).is_some_and(|q| q.ends_with('.'))
    })
}

/// Whether the class `ancestors[index]` is accessible everywhere: public (explicitly, or implicitly as a member of an
/// interface), and nested only in types that are accessible everywhere.
fn is_public_class(ancestors: &[&Tree], index: usize, context: &Context) -> bool {
    let class = ancestors[index];
    let K::Class(declaration) = &class.kind else { return false };
    let Some(container) = index.checked_sub(1).map(|i| ancestors[i]) else { return false };
    let is_public = || has_modifier(&declaration.mods, context.text, context.tokens(), "public");
    match &container.kind {
        K::CompilationUnit { .. } => is_public(),
        K::Class(outer) if is_member(outer, class) => {
            let implicitly_public = matches!(outer.kind, ClassKind::Interface | ClassKind::Annotation);
            (implicitly_public || is_public()) && is_public_class(ancestors, index - 1, context)
        }
        // Local classes, and members of anonymous classes and enum constants.
        _ => false,
    }
}

/// Whether a declaration in a class body is a member of the class, rather than of an enum constant's body.
fn is_member(class: &ClassDecl, declaration: &Tree) -> bool {
    !class.enum_constants.iter().filter_map(|c| c.body.as_deref()).flatten().any(|m| std::ptr::eq(m, declaration))
}
