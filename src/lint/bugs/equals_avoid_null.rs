//! B008: `s.equals("literal")` on a `String` variable (Checkstyle EqualsAvoidNull with its default properties, PMD
//! LiteralsFirstInComparisons): `"literal".equals(s)` cannot throw a `NullPointerException`. Also for
//! `equalsIgnoreCase`.
//!
//! As in Checkstyle, the argument must be a string literal, possibly assigned (`s.equals(t = "x")`), or a string
//! concatenation with a literal or name as a direct operand; and the receiver a name (or `this.name`,
//! `Outer.name`) whose declaration in the enclosing scopes has the type `String` (or `String[]`). Fields count
//! wherever they are declared in the class, other variables only if declared before the call. The scopes are
//! Checkstyle's: classes, enums and records; the bodies of interfaces, anonymous classes and enum constants; methods,
//! constructors and lambdas; blocks; and `for`, `try`, `catch` and `switch`.
//!
//! Unsafe fix: swaps receiver and argument (`"literal".equals(s)`). Where `s` is null, the call gives `false` instead of
//! throwing, and an assignment in the argument now happens before `s` is read.

use std::collections::HashMap;

use tree_sitter::Node;

use crate::lint::syntax::{has_comment, is_comment, named_children, operator, source, strip_parentheses};
use crate::lint::{Diagnostic, Edit, Rule};

/// The variables declared in a scope by name: the offset of the declaration and whether its type is `String`.
type Variables<'a> = HashMap<&'a str, (usize, bool)>;

pub fn check(call: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let method = call.child_by_field_name("name").map_or("", |n| source(&n, text));
    if !matches!(method, "equals" | "equalsIgnoreCase") {
        return;
    }
    let Some(arguments) = call.child_by_field_name("arguments") else { return };
    let argument_nodes: Vec<Node> = named_children(&arguments).into_iter().filter(|a| !is_comment(a)).collect();
    let ([argument], Some(object)) = (argument_nodes.as_slice(), call.child_by_field_name("object")) else { return };
    // The cheap test first: resolving the receiver walks up the tree.
    if is_literal_argument(argument, text) && is_string_variable(call, &object, text) {
        let message = format!("String literal expressions should be on the left side of an {method} comparison");
        let diagnostic = Diagnostic::new(Rule::EqualsAvoidNull, arguments.start_byte(), message);
        diagnostics.push(if has_comment(call) || call.child_by_field_name("type_arguments").is_some() {
            diagnostic
        } else {
            let receiver = match argument.kind() {
                "string_literal" | "parenthesized_expression" => source(argument, text).to_string(),
                _ => format!("({})", source(argument, text)),
            };
            let replacement = format!("{receiver}.{method}({})", source(&object, text));
            diagnostic.with_unsafe_fix(vec![Edit::new(call.byte_range(), replacement)])
        });
    }
}

/// Checkstyle's test for an argument that cannot be null.
fn is_literal_argument(argument: &Node, text: &str) -> bool {
    let mut argument = strip_parentheses(argument);
    if argument.kind() == "assignment_expression" && operator(&argument, text) == "=" {
        // Checkstyle does not look into parentheses around the assigned value.
        match argument.child_by_field_name("right") {
            Some(right) => argument = right,
            None => return false,
        }
    }
    match argument.kind() {
        "string_literal" => true,
        "binary_expression" if operator(&argument, text) == "+" => ["left", "right"]
            .iter()
            .filter_map(|field| argument.child_by_field_name(field))
            .any(|operand| matches!(strip_parentheses(&operand).kind(), "string_literal" | "identifier")),
        _ => false,
    }
}

/// Whether the receiver is a name, `this.name` or `Class.name` declared with the type `String`.
fn is_string_variable(call: &Node, object: &Node, text: &str) -> bool {
    let (name, qualifier) = match object.kind() {
        "identifier" => (*object, None),
        "field_access" => match (object.child_by_field_name("field"), object.child_by_field_name("object")) {
            (Some(field), Some(qualifier)) if matches!(qualifier.kind(), "this" | "identifier") => {
                (field, Some(qualifier))
            }
            _ => return false,
        },
        _ => return false,
    };
    let name = source(&name, text);
    // The enclosing scopes, innermost first: calls that get here are rare enough to afford `Node::parent`.
    let scopes = std::iter::successors(call.parent(), |n| n.parent()).filter_map(|n| scope(&n, text).map(|c| (n, c)));
    for (scope, class_name) in scopes {
        let declared = || variables(&scope, text).get(name).copied();
        match qualifier {
            None => {
                if let Some((offset, is_string)) = declared()
                    && (class_name.is_some() || offset < object.start_byte())
                {
                    return is_string;
                }
            }
            // `this.name` refers to the innermost class, `Class.name` to the enclosing class of that name.
            Some(qualifier) => {
                let is_class = qualifier.kind() == "this" || class_name == Some(source(&qualifier, text));
                if class_name.is_some() && is_class {
                    return declared().is_some_and(|(_, is_string)| is_string);
                }
            }
        }
    }
    false
}

/// Whether the node is a scope for fields, with the name of a class, enum or record, whose fields are
/// visible anywhere in it.
fn scope<'a>(node: &Node, text: &'a str) -> Option<Option<&'a str>> {
    match node.kind() {
        "class_declaration" | "enum_declaration" | "record_declaration" => {
            Some(Some(node.child_by_field_name("name").map_or("", |n| source(&n, text))))
        }
        "object_creation_expression" if named_children(node).iter().any(|c| c.kind() == "class_body") => Some(None),
        "interface_body"
        | "annotation_type_body"
        | "enum_constant"
        | "method_declaration"
        | "constructor_declaration"
        | "compact_constructor_declaration"
        | "lambda_expression"
        | "for_statement"
        | "enhanced_for_statement"
        | "catch_clause"
        | "try_statement"
        | "try_with_resources_statement"
        | "switch_expression"
        | "block"
        | "constructor_body" => Some(None),
        _ => None,
    }
}

/// The variables declared in the scope: by the node itself and by its descendants outside nested scopes. A later
/// declaration of a name replaces an earlier one, as in Checkstyle.
fn variables<'a>(scope_node: &Node, text: &'a str) -> Variables<'a> {
    let mut variables = Variables::new();
    declare(scope_node, text, &mut variables);
    let mut cursor = scope_node.walk();
    if !cursor.goto_first_child() {
        return variables;
    }
    loop {
        let node = cursor.node();
        let enter = node.is_named() && scope(&node, text).is_none() && {
            declare(&node, text, &mut variables);
            true
        };
        if enter && cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() || cursor.node() == *scope_node {
                return variables;
            }
        }
    }
}

/// Adds the variables the node declares.
fn declare<'a>(node: &Node, text: &'a str, variables: &mut Variables<'a>) {
    let mut add = |name: Option<Node>, type_node: Option<Node>| {
        if let Some(name) = name {
            let is_string = type_node.is_some_and(|t| is_string_type(&t, text));
            variables.insert(source(&name, text), (name.start_byte(), is_string));
        }
    };
    let type_node = || node.child_by_field_name("type");
    match node.kind() {
        "field_declaration" | "local_variable_declaration" | "constant_declaration" => {
            for declarator in node.children_by_field_name("declarator", &mut node.walk()) {
                add(declarator.child_by_field_name("name"), type_node());
            }
        }
        "formal_parameter" | "enhanced_for_statement" | "resource" | "catch_formal_parameter" => {
            add(node.child_by_field_name("name"), type_node())
        }
        "instanceof_expression" => add(node.child_by_field_name("name"), node.child_by_field_name("right")),
        "spread_parameter" => {
            let declarator = named_children(node).into_iter().find(|c| c.kind() == "variable_declarator");
            add(declarator.and_then(|d| d.child_by_field_name("name")), type_node());
        }
        // Parameters without types hide outer variables (a single one without parentheses does not in Checkstyle).
        "inferred_parameters" => {
            for name in named_children(node).into_iter().filter(|c| c.kind() == "identifier") {
                add(Some(name), None);
            }
        }
        "type_pattern" | "record_pattern_component" => {
            let children = named_children(node);
            let name = children.iter().rfind(|c| c.kind() == "identifier").copied();
            let pattern_type = children.iter().find(|c| !matches!(c.kind(), "modifiers" | "identifier")).copied();
            add(name, pattern_type);
        }
        _ => {}
    }
}

/// Whether the declared type is `String` (Checkstyle compares the simple type name, ignoring array dimensions).
fn is_string_type(type_node: &Node, text: &str) -> bool {
    let mut node = *type_node;
    while node.kind() == "array_type" {
        match node.child_by_field_name("element") {
            Some(element) => node = element,
            None => return false,
        }
    }
    node.kind() == "type_identifier" && source(&node, text) == "String"
}
