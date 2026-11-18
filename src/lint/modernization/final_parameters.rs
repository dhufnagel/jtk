//! M011: method or constructor parameter that is not `final` (Checkstyle FinalParameters with its defaults, PMD
//! MethodArgumentCouldBeFinal). Methods without a body, receiver parameters (`Foo this`) and unnamed parameters (`_`)
//! are skipped; lambda, `catch` and record parameters are not checked. The fix adds `final`, unless the body assigns
//! the name (then the parameter is reported, as by Checkstyle, but `final` would not compile).

use crate::ast::{K, Tree};
use crate::lint::tree::{Context, for_each_descendant, has_modifier};
use crate::lint::{Diagnostic, Edit, Rule};

use super::shared_a::assigned_name;

pub fn check<'t>(tree: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let K::Method(method) = &tree.kind else { return };
    let Some(body) = method.body.as_deref() else { return };
    if method.compact_ctor {
        return;
    }
    for parameter in &method.params {
        let Some(parameter_type) = parameter.ty.as_deref() else { continue };
        let name = parameter.name.as_str();
        // An annotated receiver parameter (`@A Foo this`) is parsed as a parameter named `this`.
        if matches!(name, "_" | "this") || has_modifier(&parameter.mods, context.text, context.tokens(), "final") {
            continue;
        }
        let message = format!("Parameter {name} should be final");
        let diagnostic = Diagnostic::new(Rule::FinalParameters, parameter.start, message);
        if is_assigned(body, name) {
            diagnostics.push(diagnostic);
        } else {
            let fix = Edit::new(parameter_type.start..parameter_type.start, "final ");
            diagnostics.push(diagnostic.with_fix(vec![fix]));
        }
    }
}

/// Whether the code assigns or increments a variable with the name, anywhere (also where another variable of the
/// same name hides the parameter, which only costs a fix).
fn is_assigned(body: &Tree, name: &str) -> bool {
    let mut assigned = false;
    for_each_descendant(body, &mut |tree, parent| {
        assigned |= assigned_name(tree, parent).is_some_and(|(n, _)| n == name);
        !assigned
    });
    assigned
}
