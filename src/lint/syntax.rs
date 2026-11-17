//! Helpers for reading the tree-sitter syntax tree.

use tree_sitter::Node;

pub fn source<'a>(node: &Node, text: &'a str) -> &'a str {
    &text[node.byte_range()]
}

pub fn named_children<'t>(node: &Node<'t>) -> Vec<Node<'t>> {
    node.named_children(&mut node.walk()).collect()
}

pub fn is_comment(node: &Node) -> bool {
    matches!(node.kind(), "line_comment" | "block_comment")
}

/// Whether a comment is inside the node: fixes that rewrite the node from its parts would lose it.
pub fn has_comment(node: &Node) -> bool {
    let mut found = false;
    for_each_descendant(node, &[], |n| found |= is_comment(n));
    found
}

/// The expression inside any number of parentheses.
pub fn strip_parentheses<'t>(node: &Node<'t>) -> Node<'t> {
    let mut node = *node;
    while node.kind() == "parenthesized_expression" {
        match node.named_children(&mut node.walk()).find(|c| !is_comment(c)) {
            Some(inner) => node = inner,
            None => break,
        }
    }
    node
}

pub fn modifiers<'t>(declaration: &Node<'t>) -> Option<Node<'t>> {
    declaration.named_children(&mut declaration.walk()).find(|c| c.kind() == "modifiers")
}

/// Whether the declaration has the modifier keyword (`static`, `final`, `public`, ...).
pub fn has_modifier(declaration: &Node, text: &str, keyword: &str) -> bool {
    modifiers(declaration).is_some_and(|m| m.children(&mut m.walk()).any(|c| source(&c, text) == keyword))
}

/// The operator of a binary, assignment or unary expression.
pub fn operator<'a>(expression: &Node, text: &'a str) -> &'a str {
    expression.child_by_field_name("operator").map_or("", |o| source(&o, text))
}

/// Calls `f` for the descendants of `node` in document order, without entering nodes of the `opaque` kinds (which
/// are visited themselves): code in lambdas or local classes runs in a different context.
pub fn for_each_descendant<'t>(node: &Node<'t>, opaque: &[&str], mut f: impl FnMut(&Node<'t>)) {
    let mut cursor = node.walk();
    if !cursor.goto_first_child() {
        return;
    }
    loop {
        let current = cursor.node();
        let descend = current.is_named() && {
            f(&current);
            !opaque.contains(&current.kind())
        };
        if descend && cursor.goto_first_child() {
            continue;
        }
        // The cursor cannot leave `node`, so climbing stops there.
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() || cursor.node() == *node {
                return;
            }
        }
    }
}

/// The descendants of `node` of the given kind, outside nodes of the `opaque` kinds.
pub fn descendants<'t>(node: &Node<'t>, kind: &str, opaque: &[&str]) -> Vec<Node<'t>> {
    let mut found = Vec::new();
    for_each_descendant(node, opaque, |n| {
        if n.kind() == kind {
            found.push(*n);
        }
    });
    found
}

/// Code that runs in a different context than the code around it.
pub const NESTED_CODE: &[&str] = &["lambda_expression", "class_body"];

/// Whether evaluating the expression has no side effects: names, field accesses on names, and literals.
pub fn is_side_effect_free(expression: &Node) -> bool {
    match expression.kind() {
        "identifier"
        | "this"
        | "null_literal"
        | "true"
        | "false"
        | "decimal_integer_literal"
        | "string_literal"
        | "character_literal"
        | "decimal_floating_point_literal"
        | "hex_integer_literal" => true,
        "field_access" => expression.child_by_field_name("object").is_some_and(|o| is_side_effect_free(&o)),
        "parenthesized_expression" => is_side_effect_free(&strip_parentheses(expression)),
        _ => false,
    }
}

/// Like [`for_each_descendant`] without opaque nodes, also passing each node's parent: cheaper than
/// `Node::parent`, which searches from the root.
pub fn for_each_descendant_with_parent<'t>(node: &Node<'t>, mut f: impl FnMut(&Node<'t>, &Node<'t>)) {
    let mut cursor = node.walk();
    let mut parents = vec![*node];
    if !cursor.goto_first_child() {
        return;
    }
    loop {
        let current = cursor.node();
        if current.is_named() {
            f(&current, parents.last().unwrap_or(node));
            if cursor.goto_first_child() {
                parents.push(current);
                continue;
            }
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() || cursor.node() == *node {
                return;
            }
            parents.pop();
        }
    }
}

/// Whether an identifier refers to a variable or field by its simple name, rather than declaring something, naming a
/// method, label or annotation element, or selecting a member (`a.x`).
pub fn is_name_reference(identifier: &Node) -> bool {
    identifier.parent().is_none_or(|parent| is_name_reference_in(identifier, &parent))
}

/// [`is_name_reference`] for an identifier whose parent is known.
pub fn is_name_reference_in(identifier: &Node, parent: &Node) -> bool {
    let is_field = |name| parent.child_by_field_name(name) == Some(*identifier);
    match parent.kind() {
        "field_access" => !is_field("field"),
        "method_invocation"
        | "variable_declarator"
        | "formal_parameter"
        | "catch_formal_parameter"
        | "spread_parameter"
        | "enhanced_for_statement"
        | "instanceof_expression"
        | "resource"
        | "method_declaration"
        | "constructor_declaration"
        | "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "record_declaration"
        | "annotation_type_declaration"
        | "annotation_type_element_declaration"
        | "enum_constant"
        | "type_parameter" => !is_field("name"),
        "lambda_expression" => !is_field("parameters"),
        "element_value_pair" => !is_field("key"),
        "method_reference" => parent.named_child(0) == Some(*identifier),
        "type_pattern" | "record_pattern" | "record_pattern_component" => false,
        "labeled_statement"
        | "break_statement"
        | "continue_statement"
        | "inferred_parameters"
        | "scoped_identifier"
        | "import_declaration"
        | "package_declaration"
        | "annotation"
        | "marker_annotation" => false,
        _ => true,
    }
}

/// Whether the identifier is only written: the target of a plain assignment, or incremented (`i++`) without using
/// the result. Like Checkstyle, a compound assignment (`x += 1`) counts as a use.
pub fn is_only_written(identifier: &Node, text: &str) -> bool {
    let Some(parent) = identifier.parent() else { return false };
    match parent.kind() {
        "assignment_expression" => {
            parent.child_by_field_name("left") == Some(*identifier) && operator(&parent, text) == "="
        }
        "update_expression" => {
            parent.parent().is_some_and(|p| matches!(p.kind(), "expression_statement" | "for_statement"))
        }
        _ => false,
    }
}

/// The modifier keywords of a declaration (without annotations).
pub fn modifier_keywords<'t>(declaration: &Node<'t>) -> Vec<Node<'t>> {
    let Some(modifiers) = modifiers(declaration) else { return Vec::new() };
    named_children(&modifiers).into_iter().filter(|m| matches!(m.kind(), "modifier" | "visibility")).collect()
}

/// The range that removes a token together with the whitespace after it on the same line.
pub fn removal_range(node: &Node, text: &str) -> std::ops::Range<usize> {
    let trailing = text[node.end_byte()..].len() - text[node.end_byte()..].trim_start_matches([' ', '\t']).len();
    node.start_byte()..node.end_byte() + trailing
}

/// The range that removes a node: its whole lines if nothing else is on them, otherwise the node with the spaces after
/// it.
pub fn line_removal_range(node: &Node, text: &str) -> std::ops::Range<usize> {
    span_removal_range(node.start_byte()..node.end_byte(), text)
}

/// Like [`line_removal_range`] for a span of source, such as a declaration with its leading comments.
pub fn span_removal_range(span: std::ops::Range<usize>, text: &str) -> std::ops::Range<usize> {
    let line_start = text[..span.start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[span.end..].find('\n').map_or(text.len(), |i| span.end + i + 1);
    let alone = text[line_start..span.start].trim().is_empty() && text[span.end..line_end].trim().is_empty();
    if alone {
        line_start..line_end
    } else {
        let trailing = text[span.end..].len() - text[span.end..].trim_start_matches([' ', '\t']).len();
        span.start..span.end + trailing
    }
}

/// `!expression`, with parentheses unless the expression is a primary expression.
pub fn negation(expression: &Node, text: &str) -> String {
    let expression = strip_parentheses(expression);
    let code = source(&expression, text);
    match expression.kind() {
        "identifier" | "method_invocation" | "field_access" | "array_access" | "true" | "false" | "this" => {
            format!("!{code}")
        }
        "unary_expression" if operator(&expression, text) == "!" => {
            expression.child_by_field_name("operand").map_or(format!("!({code})"), |o| source(&o, text).to_string())
        }
        _ => format!("!({code})"),
    }
}

pub fn is_boolean_literal(node: &Node) -> bool {
    matches!(node.kind(), "true" | "false")
}

/// The declared type of the local variable, parameter or field `name` visible at `node`: the `type` node of its
/// declaration. `None` if no declaration is found in the enclosing code, if the name is declared with extra array
/// dimensions (`int a[]`) or as an implicitly typed lambda parameter, and if it may be a field inherited by an
/// enclosing class (which has a supertype and does not declare the name itself). Climbs with `Node::parent`, so it
/// is meant for the few candidates a rule has already found.
pub fn declared_type<'t>(node: &Node<'t>, name: &str, text: &str) -> Option<Node<'t>> {
    let declares = |n: &Node| n.child_by_field_name("name").is_some_and(|n| source(&n, text) == name);
    let unless_dimensions =
        |n: &Node<'t>| n.child_by_field_name("type").filter(|_| n.child_by_field_name("dimensions").is_none());
    let mut child = *node;
    while let Some(parent) = child.parent() {
        let found = match parent.kind() {
            "block" | "constructor_body" | "switch_block_statement_group" => named_children(&parent)
                .iter()
                .filter(|m| m.end_byte() <= child.start_byte())
                .filter_map(|m| declaration_type(m, name, text))
                .next_back(),
            "class_body" | "interface_body" | "enum_body_declarations" => {
                named_children(&parent).iter().find_map(|m| declaration_type(m, name, text))
            }
            "switch_block" => named_children(&parent)
                .iter()
                .filter(|group| group.end_byte() <= child.start_byte())
                .flat_map(named_children)
                .filter_map(|m| declaration_type(&m, name, text))
                .next_back(),
            "for_statement" => parent.child_by_field_name("init").and_then(|i| declaration_type(&i, name, text)),
            "enhanced_for_statement" if declares(&parent) => unless_dimensions(&parent),
            "resource_specification" => named_children(&parent)
                .iter()
                .filter(|r| r.end_byte() <= child.start_byte() && declares(r))
                .find_map(unless_dimensions),
            "try_with_resources_statement" => parent
                .child_by_field_name("resources")
                .and_then(|r| named_children(&r).iter().filter(|r| declares(r)).find_map(unless_dimensions)),
            "catch_clause" => named_children(&parent)
                .into_iter()
                .find(|p| p.kind() == "catch_formal_parameter" && declares(p))
                .and_then(|p| named_children(&p).into_iter().find(|t| t.kind() == "catch_type")),
            "method_declaration" | "constructor_declaration" | "record_declaration" => {
                parent.child_by_field_name("parameters").and_then(|p| parameter_type(&p, name, text))
            }
            "lambda_expression" => match parent.child_by_field_name("parameters") {
                Some(p) if p.kind() == "formal_parameters" => parameter_type(&p, name, text),
                Some(p) if p.kind() == "identifier" && source(&p, text) == name => return None,
                Some(p) if named_children(&p).iter().any(|i| source(i, text) == name) => return None,
                _ => None,
            },
            _ => None,
        };
        if found.is_some() {
            return found;
        }
        let may_inherit = match parent.kind() {
            "class_declaration" | "enum_declaration" | "record_declaration" => {
                parent.child_by_field_name("superclass").is_some() || parent.child_by_field_name("interfaces").is_some()
            }
            "interface_declaration" => named_children(&parent).iter().any(|c| c.kind() == "extends_interfaces"),
            "object_creation_expression" => child.kind() == "class_body",
            _ => false,
        };
        if may_inherit {
            return None;
        }
        child = parent;
    }
    None
}

/// The type of a local variable or field declaration that declares `name` without extra array dimensions.
fn declaration_type<'t>(declaration: &Node<'t>, name: &str, text: &str) -> Option<Node<'t>> {
    if !matches!(declaration.kind(), "local_variable_declaration" | "field_declaration" | "constant_declaration") {
        return None;
    }
    let mut declarators = declaration.children_by_field_name("declarator", &mut declaration.walk()).collect::<Vec<_>>();
    declarators.retain(|d| d.child_by_field_name("name").is_some_and(|n| source(&n, text) == name));
    let declarator = declarators.first()?;
    declarator.child_by_field_name("dimensions").is_none().then(|| declaration.child_by_field_name("type")).flatten()
}

/// The type of the parameter `name` in a parameter list.
fn parameter_type<'t>(parameters: &Node<'t>, name: &str, text: &str) -> Option<Node<'t>> {
    named_children(parameters).into_iter().find_map(|p| match p.kind() {
        "formal_parameter" if p.child_by_field_name("name").is_some_and(|n| source(&n, text) == name) => {
            p.child_by_field_name("type").filter(|_| p.child_by_field_name("dimensions").is_none())
        }
        _ => None,
    })
}

/// The name of a formal parameter, or of a variable arity parameter (`String... names`).
pub fn parameter_name<'t>(parameter: &Node<'t>) -> Option<Node<'t>> {
    match parameter.kind() {
        "formal_parameter" => parameter.child_by_field_name("name"),
        _ => named_children(parameter)
            .into_iter()
            .find(|c| c.kind() == "variable_declarator")
            .and_then(|d| d.child_by_field_name("name")),
    }
}
