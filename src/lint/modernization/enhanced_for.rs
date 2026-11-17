//! M007: `for` loop over the indexes of an array or list that can be an enhanced `for` loop (PMD
//! ForLoopCanBeForeach, Eclipse save action "Convert to enhanced for loop"), Java 5.
//!
//! As in PMD: `for (int i = 0; i < a.length; i++)` (also `i <= a.length - 1`, `++i`, `i += 1`, `i = i + 1`) where
//! the body uses `i` only to read `a[i]`, or `list.get(i)` with `i < list.size()`; and loops over an iterator
//! (`for (Iterator<T> it = c.iterator(); it.hasNext();)`) that use it only for `it.next()`. Without type information
//! the array or list must be a variable, parameter or field declared in the file with an array type or a `List` type
//! of `java.util`; PMD also knows inherited fields and other `List` types.
//!
//! The fix turns an index loop into `for (T e : a)` if the element type `T` is known (the array's element type, the
//! list's type argument) and the body does not use the array or list otherwise (so the array cannot change, nor the
//! list be modified, which would make the iterator throw). The element is named after the array or list (`items`:
//! `item`), or `element` if that name is taken or not a plural.

use tree_sitter::Node;

use crate::lint::syntax::{has_comment, is_name_reference_in, named_children, operator, source};
use crate::lint::{Diagnostic, Edit, Rule};

const LIST_TYPES: &[&str] = &[
    "List",
    "ArrayList",
    "LinkedList",
    "Vector",
    "Stack",
    "CopyOnWriteArrayList",
    "AbstractList",
    "AbstractSequentialList",
];

/// The `java.util` types whose iterator loops are reported (and `Iterable`).
const COLLECTION_TYPES: &[&str] = &[
    "Iterable",
    "Collection",
    "List",
    "ArrayList",
    "LinkedList",
    "Vector",
    "Stack",
    "Set",
    "HashSet",
    "LinkedHashSet",
    "TreeSet",
    "SortedSet",
    "NavigableSet",
    "Queue",
    "Deque",
    "ArrayDeque",
    "PriorityQueue",
];

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Array,
    List,
}

pub fn check(loop_node: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let message = "This 'for' loop can be replaced by an enhanced 'for' loop";
    let Some(init) = loop_node.child_by_field_name("init") else {
        // `int i = 0; for (; i < a.length; i++)`: reported, not fixed.
        if index_loop(loop_node, None, text).is_some() {
            diagnostics.push(Diagnostic::new(Rule::EnhancedFor, loop_node.start_byte(), message));
        }
        return;
    };
    if init.kind() != "local_variable_declaration" {
        return;
    }
    let declarators: Vec<Node> =
        named_children(&init).into_iter().filter(|d| d.kind() == "variable_declarator").collect();
    let [declarator] = declarators.as_slice() else { return };
    let (Some(index), Some(value), Some(index_type)) = (
        declarator.child_by_field_name("name"),
        declarator.child_by_field_name("value"),
        init.child_by_field_name("type"),
    ) else {
        return;
    };
    let index_name = source(&index, text);
    if index_type.kind() == "integral_type" && source(&index_type, text) == "int" && source(&value, text) == "0" {
        let Some(fix) = index_loop(loop_node, Some(index_name), text) else { return };
        let diagnostic = Diagnostic::new(Rule::EnhancedFor, loop_node.start_byte(), message);
        diagnostics.push(match fix {
            Some(edits) => diagnostic.with_fix(edits),
            None => diagnostic,
        });
    } else if is_iterator_loop(loop_node, index_name, &index_type, &value, text) {
        diagnostics.push(Diagnostic::new(Rule::EnhancedFor, loop_node.start_byte(), message));
    }
}

/// Whether the loop iterates over the indexes of an array or list (`None` if not), with the fix if there is one.
/// Without the index declared in the loop (`index` is `None`), it must be an `int` variable declared in the block
/// around the loop, and used only in the loop.
fn index_loop(loop_node: &Node, index: Option<&str>, text: &str) -> Option<Option<Vec<Edit>>> {
    let fixable = index.is_some();
    let (index, scope) = match index {
        Some(index) => (index, loop_node.child_by_field_name("body")?),
        None => {
            let index = incremented_variable(loop_node, text)?;
            let block = loop_node.parent().filter(|p| matches!(p.kind(), "block" | "constructor_body"))?;
            let declared_int = named_children(&block)
                .into_iter()
                .take_while(|c| c.start_byte() < loop_node.start_byte())
                .filter(|c| c.kind() == "local_variable_declaration")
                .filter_map(|d| declarator_type(&d, index, text))
                .last()
                .is_some_and(|d| d.extra_dimensions == 0 && source(&d.type_node, text) == "int");
            if !declared_int {
                return None;
            }
            (index, block)
        }
    };
    let (iterable, kind) = iterable_of_condition(loop_node, index, text)?;
    if incremented_variable(loop_node, text) != Some(index) {
        return None;
    }
    let accesses = element_accesses(loop_node, &scope, index, &iterable, kind, text)?;
    let declared = declared_type(loop_node, &iterable, text)?;
    let element_type = element_type(&declared, kind, text)?;
    if kind == Kind::List && !is_java_util_type(&declared.type_node, text) {
        return None;
    }
    Some(element_type.filter(|_| fixable).and_then(|t| enhanced_for(loop_node, &iterable, &t, &accesses, text)))
}
/// The iterated array or list: a name or `this.name`.
struct Iterable<'a> {
    name: &'a str,
    qualified: bool,
    code: &'a str,
}

/// The array or list of the condition `i < a.length`, `i <= a.length - 1` or `i < list.size()`.
fn iterable_of_condition<'a>(loop_node: &Node, index: &str, text: &'a str) -> Option<(Iterable<'a>, Kind)> {
    let condition = loop_node.child_by_field_name("condition").filter(|c| c.kind() == "binary_expression")?;
    let left = condition.child_by_field_name("left")?;
    let right = condition.child_by_field_name("right")?;
    if left.kind() != "identifier" || source(&left, text) != index {
        return None;
    }
    let bound = match operator(&condition, text) {
        "<" => right,
        "<=" => {
            let one = right.child_by_field_name("right")?;
            if right.kind() != "binary_expression" || operator(&right, text) != "-" || source(&one, text) != "1" {
                return None;
            }
            right.child_by_field_name("left")?
        }
        _ => return None,
    };
    let (object, kind) = match bound.kind() {
        "field_access" if bound.child_by_field_name("field").is_some_and(|f| source(&f, text) == "length") => {
            (bound.child_by_field_name("object")?, Kind::Array)
        }
        "method_invocation"
            if bound.child_by_field_name("name").is_some_and(|n| source(&n, text) == "size")
                && bound.child_by_field_name("arguments").is_some_and(|a| a.named_child_count() == 0)
                && bound.child_by_field_name("type_arguments").is_none() =>
        {
            (bound.child_by_field_name("object")?, Kind::List)
        }
        _ => return None,
    };
    let iterable = match object.kind() {
        "identifier" => Iterable { name: source(&object, text), qualified: false, code: source(&object, text) },
        "field_access" if object.child_by_field_name("object").is_some_and(|o| o.kind() == "this") => {
            let field = object.child_by_field_name("field").filter(|f| f.kind() == "identifier")?;
            Iterable { name: source(&field, text), qualified: true, code: source(&object, text) }
        }
        _ => return None,
    };
    Some((iterable, kind))
}

/// The variable that the loop's only update increments by one.
fn incremented_variable<'a>(loop_node: &Node, text: &'a str) -> Option<&'a str> {
    let updates: Vec<Node> = loop_node.children_by_field_name("update", &mut loop_node.walk()).collect();
    let [update] = updates.as_slice() else { return None };
    let name = |node: Option<Node>| node.filter(|n| n.kind() == "identifier").map(|n| source(&n, text));
    let is_one = |node: Option<Node>| node.is_some_and(|n| source(&n, text) == "1");
    match update.kind() {
        "update_expression" if source(update, text).contains("++") => name(update.named_child(0)),
        "assignment_expression" => {
            let left = update.child_by_field_name("left");
            let right = update.child_by_field_name("right");
            match operator(update, text) {
                "+=" if is_one(right) => name(left),
                "=" => {
                    let variable = name(left)?;
                    let is_variable = |node: Option<Node>| name(node) == Some(variable);
                    let r = right.filter(|r| r.kind() == "binary_expression" && operator(r, text) == "+")?;
                    let (a, b) = (r.child_by_field_name("left"), r.child_by_field_name("right"));
                    (is_variable(a) && is_one(b) || is_one(a) && is_variable(b)).then_some(variable)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Whether the declared type is a type of `java.util`: written qualified, or imported by name or with `*`.
fn is_java_util_type(type_node: &Node, text: &str) -> bool {
    let base = if type_node.kind() == "generic_type" { type_node.named_child(0) } else { Some(*type_node) };
    let Some(base) = base.map(|b| source(&b, text)) else { return false };
    if let Some(simple) = base.strip_prefix("java.util.") {
        return !simple.contains('.');
    }
    !base.contains('.') && (text.contains("import java.util.*;") || text.contains(&format!("import java.util.{base};")))
}

/// The element accesses `a[i]` or `list.get(i)` in the body, if they are the only uses of the index and only read
/// the element.
fn element_accesses<'t>(
    loop_node: &Node<'t>,
    scope: &Node<'t>,
    index: &str,
    iterable: &Iterable,
    kind: Kind,
    text: &str,
) -> Option<Vec<Node<'t>>> {
    let body = *scope;
    let loop_body = loop_node.child_by_field_name("body")?.byte_range();
    // In the scope of an index declared before the loop, its uses in the loop's condition and update.
    let condition = loop_node.child_by_field_name("condition")?.byte_range();
    let update = loop_node.child_by_field_name("update")?.byte_range();
    let mut accesses = Vec::new();
    let mut ancestors = vec![body];
    let mut cursor = body.walk();
    let mut valid = true;
    // Iterative walk keeping the ancestors, as `Node::parent` is slow.
    'walk: loop {
        let node = cursor.node();
        if node.kind() == "identifier" && source(&node, text) == index {
            let parent = ancestors[ancestors.len().saturating_sub(2)];
            let position = node.start_byte();
            let in_header = condition.contains(&position) || update.contains(&position);
            if node != body && !in_header && is_name_reference_in(&node, &parent) {
                let grandparent = ancestors.len().checked_sub(3).map(|i| ancestors[i]);
                match element_access(&node, &parent, grandparent, iterable, kind, text) {
                    Some(access) if loop_body.contains(&position) => accesses.push(access),
                    _ => valid = false,
                }
            }
        }
        if !valid {
            return None;
        }
        if cursor.goto_first_child() {
            ancestors.push(cursor.node());
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                *ancestors.last_mut().unwrap() = cursor.node();
                continue 'walk;
            }
            if !cursor.goto_parent() || cursor.node() == body {
                break 'walk;
            }
            ancestors.pop();
        }
    }
    Some(accesses)
}

/// The access `a[i]` or `list.get(i)` of the index identifier, if it reads the element.
fn element_access<'t>(
    index: &Node<'t>,
    parent: &Node<'t>,
    grandparent: Option<Node<'t>>,
    iterable: &Iterable,
    kind: Kind,
    text: &str,
) -> Option<Node<'t>> {
    match kind {
        Kind::Array => {
            let is_access = parent.kind() == "array_access"
                && parent.child_by_field_name("index") == Some(*index)
                && parent.child_by_field_name("array").is_some_and(|a| source(&a, text) == iterable.code);
            let is_written = grandparent.is_some_and(|g| match g.kind() {
                "assignment_expression" => g.child_by_field_name("left") == Some(*parent),
                "update_expression" => true,
                _ => false,
            });
            (is_access && !is_written).then_some(*parent)
        }
        Kind::List => {
            let call = grandparent.filter(|g| g.kind() == "method_invocation")?;
            let is_get = parent.kind() == "argument_list"
                && parent.named_child_count() == 1
                && call.child_by_field_name("name").is_some_and(|n| source(&n, text) == "get")
                && call.child_by_field_name("object").is_some_and(|o| source(&o, text) == iterable.code);
            is_get.then_some(call)
        }
    }
}

/// The declared type of the array or list: the declaration of a local variable, parameter or field in the file.
struct Declared<'t> {
    type_node: Node<'t>,
    /// Array dimensions after the variable name (`int a[]`), and one for a variable arity parameter.
    extra_dimensions: usize,
}

fn declared_type<'t>(loop_node: &Node<'t>, iterable: &Iterable, text: &str) -> Option<Declared<'t>> {
    let mut child = *loop_node;
    let mut ancestor = loop_node.parent();
    while let Some(node) = ancestor {
        let found = match node.kind() {
            "block" | "constructor_body" | "switch_block_statement_group" if !iterable.qualified => {
                named_children(&node)
                    .into_iter()
                    .take_while(|c| c.start_byte() < child.start_byte())
                    .filter(|c| c.kind() == "local_variable_declaration")
                    .find_map(|d| declarator_type(&d, iterable.name, text))
            }
            "for_statement" if !iterable.qualified => node
                .child_by_field_name("init")
                .filter(|i| i.kind() == "local_variable_declaration")
                .and_then(|d| declarator_type(&d, iterable.name, text)),
            "enhanced_for_statement" if !iterable.qualified => {
                let name = node.child_by_field_name("name");
                let declared = name.is_some_and(|n| source(&n, text) == iterable.name);
                let dimensions = node.child_by_field_name("dimensions").map_or(0, |d| dimensions(&d, text));
                node.child_by_field_name("type")
                    .filter(|_| declared)
                    .map(|type_node| Declared { type_node, extra_dimensions: dimensions })
            }
            "method_declaration" | "constructor_declaration" | "lambda_expression" if !iterable.qualified => node
                .child_by_field_name("parameters")
                .filter(|p| p.kind() == "formal_parameters")
                .and_then(|p| parameter_type(&p, iterable.name, text)),
            "class_body" | "enum_body_declarations" => {
                let field = named_children(&node)
                    .into_iter()
                    .filter(|c| c.kind() == "field_declaration")
                    .find_map(|d| declarator_type(&d, iterable.name, text));
                if field.is_some() || iterable.qualified {
                    return field;
                }
                None
            }
            "program" => return None,
            _ => None,
        };
        if found.is_some() {
            return found;
        }
        child = node;
        ancestor = node.parent();
    }
    None
}

fn declarator_type<'t>(declaration: &Node<'t>, name: &str, text: &str) -> Option<Declared<'t>> {
    let declarator = named_children(declaration).into_iter().find(|d| {
        d.kind() == "variable_declarator" && d.child_by_field_name("name").is_some_and(|n| source(&n, text) == name)
    })?;
    let extra_dimensions = declarator.child_by_field_name("dimensions").map_or(0, |d| dimensions(&d, text));
    Some(Declared { type_node: declaration.child_by_field_name("type")?, extra_dimensions })
}

fn parameter_type<'t>(parameters: &Node<'t>, name: &str, text: &str) -> Option<Declared<'t>> {
    named_children(parameters).into_iter().find_map(|p| match p.kind() {
        "formal_parameter" if p.child_by_field_name("name").is_some_and(|n| source(&n, text) == name) => {
            let extra_dimensions = p.child_by_field_name("dimensions").map_or(0, |d| dimensions(&d, text));
            Some(Declared { type_node: p.child_by_field_name("type")?, extra_dimensions })
        }
        "spread_parameter" => {
            let declarator = named_children(&p).into_iter().find(|c| c.kind() == "variable_declarator")?;
            let named = declarator.child_by_field_name("name").is_some_and(|n| source(&n, text) == name);
            named.then_some(Declared { type_node: p.child_by_field_name("type")?, extra_dimensions: 1 })
        }
        _ => None,
    })
}

fn dimensions(node: &Node, text: &str) -> usize {
    source(node, text).matches('[').count()
}

/// Whether the declared type is an array or list type (`None` if not); with the element type if it is known.
fn element_type(declared: &Declared, kind: Kind, text: &str) -> Option<Option<String>> {
    let type_node = declared.type_node;
    match kind {
        Kind::Array => {
            let (element, type_dimensions) = if type_node.kind() == "array_type" {
                let element = type_node.child_by_field_name("element")?;
                let dimensions = type_node.child_by_field_name("dimensions").map_or(0, |d| dimensions(&d, text));
                (element, dimensions)
            } else {
                (type_node, 0)
            };
            let total = type_dimensions + declared.extra_dimensions;
            if total == 0 {
                return None;
            }
            let annotated = source(&type_node, text).contains('@');
            let element = format!("{}{}", source(&element, text), "[]".repeat(total - 1));
            Some((!annotated && source(&type_node, text) != "var").then_some(element))
        }
        Kind::List => {
            if declared.extra_dimensions > 0 {
                return None;
            }
            let (base, arguments) = match type_node.kind() {
                "generic_type" => {
                    let children = named_children(&type_node);
                    (*children.first()?, children.into_iter().find(|c| c.kind() == "type_arguments"))
                }
                "type_identifier" | "scoped_type_identifier" => (type_node, None),
                _ => return None,
            };
            let base = source(&base, text);
            let simple = base.rsplit('.').next().unwrap_or(base);
            let is_list = LIST_TYPES.contains(&simple) && (simple == base || base.starts_with("java.util."));
            if !is_list {
                return None;
            }
            let argument = arguments.and_then(|a| match named_children(&a).as_slice() {
                [argument] if argument.kind() != "wildcard" && !has_comment(&a) => {
                    Some(source(argument, text).to_string())
                }
                _ => None,
            });
            Some(argument)
        }
    }
}

/// The edits that turn the loop into `for (T e : iterable)`, if the body uses the iterable only in the element
/// accesses.
fn enhanced_for(
    loop_node: &Node,
    iterable: &Iterable,
    element_type: &str,
    accesses: &[Node],
    text: &str,
) -> Option<Vec<Edit>> {
    let body = loop_node.child_by_field_name("body")?;
    let mut other_use = false;
    crate::lint::syntax::for_each_descendant(&body, &[], |node| {
        let in_access = accesses.iter().any(|a| a.start_byte() <= node.start_byte() && node.end_byte() <= a.end_byte());
        other_use |= node.kind() == "identifier" && source(node, text) == iterable.name && !in_access;
    });
    let open = loop_node.children(&mut loop_node.walk()).find(|c| c.kind() == "(")?;
    let close = loop_node.children(&mut loop_node.walk()).find(|c| c.kind() == ")")?;
    let header = open.start_byte()..close.end_byte();
    let header_has_comment = loop_node
        .children(&mut loop_node.walk())
        .any(|c| c.start_byte() >= header.start && c.end_byte() <= header.end && crate::lint::syntax::is_comment(&c));
    if other_use || header_has_comment || has_comment(&loop_node.child_by_field_name("condition")?) {
        return None;
    }
    let name = element_name(iterable.name, text);
    // One edit for the whole loop, so that a nested loop is converted in a later round, when this loop's element
    // name is in the text.
    let mut replacements = vec![(header, format!("({element_type} {name} : {})", iterable.code))];
    replacements.extend(accesses.iter().map(|a| (a.byte_range(), name.clone())));
    let mut code = String::new();
    let mut position = loop_node.start_byte();
    for (range, replacement) in replacements {
        code.push_str(&text[position..range.start]);
        code.push_str(&replacement);
        position = range.end;
    }
    code.push_str(&text[position..loop_node.end_byte()]);
    Some(vec![Edit::new(loop_node.byte_range(), code)])
}

/// The singular of the iterable's name, or `element`; with a number if the name occurs in the file.
fn element_name(iterable: &str, text: &str) -> String {
    let singular = if let Some(stem) = iterable.strip_suffix("ies") {
        format!("{stem}y")
    } else if ["sses", "xes", "ches", "shes"].iter().any(|s| iterable.ends_with(s)) {
        iterable[..iterable.len() - 2].to_string()
    } else if iterable.ends_with('s') && !iterable.ends_with("ss") {
        iterable[..iterable.len() - 1].to_string()
    } else {
        String::new()
    };
    let is_free = |name: &str| !name.is_empty() && !JAVA_KEYWORDS.contains(&name) && !occurs_as_word(name, text);
    if singular.len() > 1 && is_free(&singular) {
        return singular;
    }
    (1..).map(|n| if n == 1 { "element".to_string() } else { format!("element{n}") }).find(|n| is_free(n)).unwrap()
}

/// Whether the name occurs in the text as a whole word: then it may be in scope.
fn occurs_as_word(name: &str, text: &str) -> bool {
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices(name).any(|(i, _)| {
        !text[..i].chars().next_back().is_some_and(is_word)
            && !text[i + name.len()..].chars().next().is_some_and(is_word)
    })
}

const JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "try",
    "void",
    "volatile",
    "while",
    "true",
    "false",
    "null",
    "var",
    "yield",
    "record",
    "sealed",
    "permits",
];

/// `for (Iterator<T> it = c.iterator(); it.hasNext();)` that uses `it` only in the condition and one `it.next()`.
fn is_iterator_loop(loop_node: &Node, iterator: &str, iterator_type: &Node, value: &Node, text: &str) -> bool {
    let base = match iterator_type.kind() {
        "generic_type" => iterator_type.named_child(0),
        _ => Some(*iterator_type),
    };
    let is_iterator_type = base.is_some_and(|b| matches!(source(&b, text), "Iterator" | "java.util.Iterator"));
    let is_call = |node: &Node, method: &str| {
        node.kind() == "method_invocation"
            && node.child_by_field_name("name").is_some_and(|n| source(&n, text) == method)
            && node.child_by_field_name("arguments").is_some_and(|a| a.named_child_count() == 0)
    };
    let condition = loop_node.child_by_field_name("condition");
    let condition_is_has_next = condition.is_some_and(|c| {
        is_call(&c, "hasNext") && c.child_by_field_name("object").is_some_and(|o| source(&o, text) == iterator)
    });
    if !is_iterator_type || !is_call(value, "iterator") || !condition_is_has_next {
        return false;
    }
    // PMD needs to know that the receiver is an `Iterable`.
    let Some(collection) = value.child_by_field_name("object").filter(|o| o.kind() != "this") else {
        // `iterator()` of the class itself.
        let class = std::iter::successors(loop_node.parent(), |n| n.parent()).find(|n| n.kind() == "class_declaration");
        return class
            .and_then(|c| c.child_by_field_name("interfaces"))
            .is_some_and(|i| source(&i, text).contains("Iterable"));
    };
    let collection = match collection.kind() {
        "identifier" => Iterable { name: source(&collection, text), qualified: false, code: "" },
        "field_access" if collection.child_by_field_name("object").is_some_and(|o| o.kind() == "this") => {
            let Some(field) = collection.child_by_field_name("field") else { return false };
            Iterable { name: source(&field, text), qualified: true, code: "" }
        }
        _ => return false,
    };
    let is_collection = declared_type(loop_node, &collection, text).is_some_and(|d| {
        let base = if d.type_node.kind() == "generic_type" { d.type_node.named_child(0) } else { Some(d.type_node) };
        let base = base.map_or("", |b| source(&b, text));
        d.extra_dimensions == 0
            && COLLECTION_TYPES.contains(&base.rsplit('.').next().unwrap_or(base))
            && (is_java_util_type(&d.type_node, text) || matches!(base, "Iterable" | "java.lang.Iterable"))
    });
    if !is_collection {
        return false;
    }
    let mut uses = 0;
    let mut next_calls = 0;
    crate::lint::syntax::for_each_descendant_with_parent(loop_node, |node, parent| {
        if node.kind() == "identifier" && source(node, text) == iterator && is_name_reference_in(node, parent) {
            uses += 1;
            if is_call(parent, "next") && parent.child_by_field_name("object") == Some(*node) {
                next_calls += 1;
            }
        }
    });
    uses == 2 && next_calls == 1
}
