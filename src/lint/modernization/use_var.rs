//! M006: local variable whose type is obvious from its value (Eclipse save action "Use var"), Java 10:
//! `ArrayList<String> list = new ArrayList<String>()` becomes `var list = new ArrayList<String>()`.
//!
//! Only where `var` infers exactly the declared type, judged from the code alone: a constructor call of the declared
//! type with explicit type arguments or none (not with `<>`, from which `var` would infer `Object` arguments, and not
//! an anonymous class), a cast to the declared type, and a string literal for `String`. Not for fields, several
//! variables in one declaration, array initializers, lambdas, method references, `null` and other literals (`long
//! x = 1` would become `int`). Eclipse also converts other values whose type it knows from type information.

use crate::ast::{K, LitKind, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check<'t>(tree: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let K::Vars(vars) = &tree.kind else { return };
    let [var] = vars.as_slice() else { return };
    // Fields are declared in class bodies; `var` has no type.
    let is_field = context.parent().is_some_and(|p| matches!(p.kind, K::Class(_) | K::NewClass { .. }));
    let (Some(declared), Some(value)) = (var.ty.as_deref(), var.init.as_deref()) else { return };
    // `int a[] = ...` declares an array type with the declarator.
    if is_field || declared.end > var.name_pos {
        return;
    }
    let declared_type = without_spaces(context.source(declared));
    let obvious = match &value.kind {
        // A constructor call with type arguments of its own (`new <T>A()`) has them before the type.
        K::NewClass { type_args, ident, body: None, .. } if type_args.is_empty() => {
            let created = match &ident.kind {
                K::Annotated { base, .. } => base,
                _ => ident,
            };
            without_spaces(context.source(created)) == declared_type
        }
        K::Cast { ty, .. } => {
            !matches!(ty.kind, K::Intersection(_)) && without_spaces(context.source(ty)) == declared_type
        }
        K::Literal(LitKind::String) => declared_type == "String",
        _ => false,
    };
    if obvious && !declared_type.ends_with("<>") && !context.tokens().has_comment(declared.start..declared.end) {
        let message = "Local variable type can be replaced with 'var'";
        let fix = Edit::new(declared.start..declared.end, "var");
        diagnostics.push(Diagnostic::new(Rule::UseVar, declared.start, message).with_fix(vec![fix]));
    }
}

fn without_spaces(code: &str) -> String {
    code.chars().filter(|c| !c.is_whitespace()).collect()
}
