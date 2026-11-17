//! B002: `equals(Foo)` without `equals(Object)` (Checkstyle CovariantEquals): the covariant method does not override
//! `Object.equals`, so collections ignore it.

use tree_sitter::Node;

use crate::lint::syntax::{named_children, source};
use crate::lint::{Diagnostic, Rule};

pub fn check(body: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let equals_methods: Vec<(Node, &str)> = named_children(body)
        .into_iter()
        .filter(|m| m.kind() == "method_declaration")
        .filter(|m| m.child_by_field_name("name").is_some_and(|n| source(&n, text) == "equals"))
        .filter_map(|m| single_parameter_type(&m, text).map(|t| (m, t)))
        .collect();
    if equals_methods.iter().any(|(_, t)| matches!(*t, "Object" | "java.lang.Object")) {
        return;
    }
    // An abstract covariant equals is a declaration for subclasses, not the mistaken implementation.
    let implemented = equals_methods.into_iter().filter(|(m, _)| m.child_by_field_name("body").is_some());
    for (method, _) in implemented {
        let name = method.child_by_field_name("name").unwrap_or(method);
        diagnostics.push(Diagnostic::new(
            Rule::CovariantEquals,
            name.start_byte(),
            "Covariant equals without equals(Object)",
        ));
    }
}

fn single_parameter_type<'a>(method: &Node, text: &'a str) -> Option<&'a str> {
    let parameters = named_children(&method.child_by_field_name("parameters")?);
    match parameters.as_slice() {
        [parameter] if parameter.kind() == "formal_parameter" => {
            Some(source(&parameter.child_by_field_name("type")?, text))
        }
        _ => None,
    }
}
