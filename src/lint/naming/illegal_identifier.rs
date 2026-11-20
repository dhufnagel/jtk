//! N014: a declared name that is `var` or contains `$` (Checkstyle IllegalIdentifierName with its default format
//! `^(?!var$|\S*\$)\S+$`). Checks the names of types, methods, annotation elements, enum constants, fields,
//! variables, parameters (also of lambdas), record components and pattern variables.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Rule};
use jtk_parser::ast::{K, Tree};

const PATTERN: &str = r"^(?!var$|\S*\$)\S+$";

pub fn check(tree: &Tree, _: &Context, diagnostics: &mut Vec<Diagnostic>) {
    for_each_name(tree, |name, offset| {
        if name == "var" || name.contains('$') {
            let message = format!("Name '{name}' must match pattern '{PATTERN}'");
            diagnostics.push(Diagnostic::new(Rule::IllegalIdentifier, offset, message));
        }
    });
}

/// Calls `f` with each checked name the tree declares and its offset.
fn for_each_name(tree: &Tree, mut f: impl FnMut(&str, usize)) {
    match &tree.kind {
        K::Class(class) => {
            f(&class.name, class.name_pos);
            class.enum_constants.iter().for_each(|c| f(&c.name, c.name_pos));
            class.record_components.iter().for_each(|c| f(&c.name, c.name_pos));
        }
        K::Method(method) => {
            // Constructors are named after their class.
            if method.ret.is_some() {
                f(&method.name, method.name_pos);
            }
            method.params.iter().for_each(|p| f(&p.name, p.name_pos));
        }
        K::Vars(vars) | K::Lambda { params: vars, .. } => vars.iter().for_each(|v| f(&v.name, v.name_pos)),
        K::ForEach { var, .. } | K::BindingPattern { var } => f(&var.name, var.name_pos),
        K::Try { catches, .. } => catches.iter().for_each(|c| f(&c.param.name, c.param.name_pos)),
        _ => {}
    }
}
