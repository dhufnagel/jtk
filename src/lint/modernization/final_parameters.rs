//! M011: method or constructor parameter that is not `final` (Checkstyle FinalParameters with its defaults, PMD
//! MethodArgumentCouldBeFinal). Methods without a body, receiver parameters (`Foo this`) and unnamed parameters (`_`)
//! are skipped; lambda, `catch` and record parameters are not checked. The fix adds `final`, unless the body assigns
//! the name (then the parameter is reported, as by Checkstyle, but `final` would not compile).

use tree_sitter::Node;

use crate::lint::syntax::{for_each_descendant, has_modifier, named_children, source, strip_parentheses};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(method: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let (Some(parameters), Some(body)) = (method.child_by_field_name("parameters"), method.child_by_field_name("body"))
    else {
        return;
    };
    for parameter in named_children(&parameters) {
        let name = match parameter.kind() {
            "formal_parameter" => parameter.child_by_field_name("name"),
            "spread_parameter" => named_children(&parameter)
                .into_iter()
                .find(|c| c.kind() == "variable_declarator")
                .and_then(|d| d.child_by_field_name("name")),
            _ => None,
        };
        let (Some(name), Some(parameter_type)) = (name, parameter.child_by_field_name("type")) else { continue };
        let name = source(&name, text);
        // tree-sitter parses an annotated receiver parameter as a parameter named `this`.
        if matches!(name, "_" | "this") || has_modifier(&parameter, text, "final") {
            continue;
        }
        let message = format!("Parameter {name} should be final");
        let diagnostic = Diagnostic::new(Rule::FinalParameters, parameter.start_byte(), message);
        if is_assigned(&body, name, text) {
            diagnostics.push(diagnostic);
        } else {
            let fix = Edit::new(parameter_type.start_byte()..parameter_type.start_byte(), "final ");
            diagnostics.push(diagnostic.with_fix(vec![fix]));
        }
    }
}

/// Whether the code assigns or increments a variable with the name, anywhere (also where another variable of the
/// same name hides the parameter, which only costs a fix).
fn is_assigned(body: &Node, name: &str, text: &str) -> bool {
    let mut assigned = false;
    for_each_descendant(body, &[], |node| {
        let target = match node.kind() {
            "assignment_expression" => node.child_by_field_name("left"),
            "update_expression" => named_children(node).into_iter().next(),
            _ => None,
        };
        assigned |= target.is_some_and(|t| {
            let t = strip_parentheses(&t);
            t.kind() == "identifier" && source(&t, text) == name
        });
    });
    assigned
}
