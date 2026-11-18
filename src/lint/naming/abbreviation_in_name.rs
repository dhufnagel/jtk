//! N013: more than four consecutive capital letters in a name (Checkstyle AbbreviationAsWordInName, by default with
//! `allowed-abbreviation-length` 3, so `parseXMLDoc` passes but `XMLHTTPRequest` does not).
//!
//! As in Checkstyle, types, methods, annotation elements, fields, local variables, parameters, record components and
//! pattern variables are checked; `static` and `final` fields and variables, interface fields and `@Override`
//! methods are not.

use super::{is_annotation_element, is_field, is_interface_member, is_static_or_final};
use crate::ast::{K, Mods, Tree, VarDecl};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    // An abbreviation and the capital starting the next word.
    let allowed_capitals = context.options.abbreviation_in_name.allowed_abbreviation_length as usize + 1;
    for_each_name(tree, context, |name, offset| {
        if has_long_abbreviation(name, allowed_capitals) {
            let message = format!(
                "Abbreviation in name '{name}' must contain no more than '{allowed_capitals}' consecutive capital letters"
            );
            diagnostics.push(Diagnostic::new(Rule::AbbreviationInName, offset, message));
        }
    });
}

/// Calls `f` with each checked name the tree declares and its offset.
fn for_each_name(tree: &Tree, context: &Context, mut f: impl FnMut(&str, usize)) {
    // A receiver parameter `@A T this` is parsed as a parameter named `this`.
    let parameters = |params: &[VarDecl], f: &mut dyn FnMut(&str, usize)| {
        params.iter().filter(|p| p.name != "this").for_each(|p| f(&p.name, p.name_pos));
    };
    match &tree.kind {
        K::Class(class) => {
            f(&class.name, class.name_pos);
            parameters(&class.record_components, &mut f);
        }
        K::Method(method) => {
            // Constructors are named after their class.
            if method.ret.is_some() && (is_annotation_element(context) || !is_override(&method.mods, context)) {
                f(&method.name, method.name_pos);
            }
            parameters(&method.params, &mut f);
        }
        K::Vars(vars) => {
            let is_constant = is_field(context) && is_interface_member(context);
            if !is_constant && !vars.first().is_some_and(|v| is_static_or_final(&v.mods, context)) {
                vars.iter().for_each(|v| f(&v.name, v.name_pos));
            }
        }
        K::ForEach { var, .. } => {
            if !is_static_or_final(&var.mods, context) {
                f(&var.name, var.name_pos);
            }
        }
        // Checkstyle skips the parameter of a lambda without parentheses (`x -> ...`).
        K::Lambda { params, .. } if context.text[tree.start..].starts_with('(') => parameters(params, &mut f),
        K::Try { catches, .. } => {
            catches.iter().filter(|c| c.param.name != "this").for_each(|c| f(&c.param.name, c.param.name_pos));
        }
        K::BindingPattern { var } => f(&var.name, var.name_pos),
        _ => {}
    }
}

/// Checkstyle only recognizes `@Override` by its simple name.
fn is_override(mods: &Mods, context: &Context) -> bool {
    mods.annotations.iter().any(|a| matches!(&a.kind, K::Annotation { ty, .. } if context.source(ty) == "Override"))
}

/// Whether the name has a run of more than `allowed_capitals` capitals, followed
/// by a lowercase letter (whose capital starts the next word, `XMLHTTPRequest`), by `_` or by nothing.
fn has_long_abbreviation(name: &str, allowed_capitals: usize) -> bool {
    let mut run = 0;
    for c in name.chars() {
        run = if c.is_uppercase() { run + 1 } else { 0 };
        if run > allowed_capitals {
            return true;
        }
    }
    false
}
