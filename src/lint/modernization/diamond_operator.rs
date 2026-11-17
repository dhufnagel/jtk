//! M004: type arguments of a constructor call that the target type implies (PMD UseDiamondOperator, Eclipse
//! "Remove redundant type arguments"), Java 7: `List<String> l = new ArrayList<String>()` becomes
//! `new ArrayList<>()`, and the raw `new ArrayList()` becomes `new ArrayList<>()`.
//!
//! Without type resolution, only creations whose target type is declared in the code are reported: initializers of
//! variables and fields, assignments to a local variable, parameter or field declared in the enclosing code, and the
//! value of a `return` in a method (not a lambda). Left out, because inference may give another type or pick another
//! overload: method arguments, conditional expressions (not poly expressions before Java 8), anonymous classes (no
//! diamond before Java 9), variables declared with `var`, and creations with explicit constructor type arguments
//! (`new <T> Foo<String>()`). With constructor arguments, the type arguments are only reported if they are exactly
//! those of the target type, so that the arguments cannot make inference give another type, and without wildcards:
//! Java 7 infers `new WeakReference<>(String.class)` from the argument alone, which does not match
//! `WeakReference<Class<?>>`.
//!
//! PMD resolves types to know which raw types are generic; jtk only reports raw creations of the generic collections
//! of `java.util` and `java.util.concurrent` (and `ThreadLocal`), imported by a single-type import or written fully
//! qualified, without constructor arguments unless the target type is raw as well. An on-demand import
//! (`java.util.*`) counts, although a class of the same name in the same package would take precedence.

use tree_sitter::Node;

use crate::lint::syntax::{declared_type, for_each_descendant, is_comment, named_children, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(node: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    match node.kind() {
        "local_variable_declaration" | "field_declaration" | "constant_declaration" => {
            let Some(target) = node.child_by_field_name("type") else { return };
            for declarator in node.children_by_field_name("declarator", &mut node.walk()) {
                if let Some(value) = declarator.child_by_field_name("value")
                    && declarator.child_by_field_name("dimensions").is_none()
                {
                    check_creation(&value, &target, text, diagnostics);
                }
            }
        }
        "assignment_expression" => {
            let (Some(left), Some(value)) = (node.child_by_field_name("left"), node.child_by_field_name("right"))
            else {
                return;
            };
            if value.kind() != "object_creation_expression"
                || node.child_by_field_name("operator").is_none_or(|o| o.kind() != "=")
            {
                return;
            }
            let target = match left.kind() {
                "identifier" => declared_type(&left, source(&left, text), text),
                "field_access" if left.child_by_field_name("object").is_some_and(|o| o.kind() == "this") => {
                    left.child_by_field_name("field").and_then(|f| field_type(node, source(&f, text), text))
                }
                _ => None,
            };
            if let Some(target) = target {
                check_creation(&value, &target, text, diagnostics);
            }
        }
        "return_statement" => {
            let Some(value) = node.named_child(0).filter(|v| v.kind() == "object_creation_expression") else { return };
            if let Some(target) = return_type(node) {
                check_creation(&value, &target, text, diagnostics);
            }
        }
        _ => {}
    }
}

fn check_creation(value: &Node, target: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if value.kind() != "object_creation_expression"
        || value.child_by_field_name("type_arguments").is_some()
        || named_children(value).iter().any(|c| c.kind() == "class_body")
        || source(target, text) == "var"
    {
        return;
    }
    let Some(created) = value.child_by_field_name("type") else { return };
    let has_arguments = value.child_by_field_name("arguments").is_some_and(|a| a.named_child_count() > 0);
    let target_arguments = type_arguments(target);
    if created.kind() == "generic_type" {
        let Some(arguments) = type_arguments(&created).filter(|a| a.named_child_count() > 0) else { return };
        let Some(target_arguments) = target_arguments else { return };
        let target_arguments = normalized(&target_arguments, text);
        if has_arguments && (normalized(&arguments, text) != target_arguments || target_arguments.contains('?')) {
            return;
        }
        let message = "Type arguments can be inferred: use <>";
        let diagnostic = Diagnostic::new(Rule::DiamondOperator, arguments.start_byte(), message);
        let mut has_comment = false;
        for_each_descendant(&arguments, &[], |n| has_comment |= is_comment(n));
        diagnostics.push(if has_comment {
            diagnostic
        } else {
            diagnostic.with_fix(vec![Edit::new(arguments.byte_range(), "<>")])
        });
    } else if (!has_arguments || target_arguments.is_none()) && is_known_generic(&created, text) {
        let fix = Edit::new(created.end_byte()..created.end_byte(), "<>");
        let message = "Raw type can be avoided: use <>";
        diagnostics.push(Diagnostic::new(Rule::DiamondOperator, created.start_byte(), message).with_fix(vec![fix]));
    }
}

/// The type arguments of a generic type (`List<String>`, `Map.Entry<K, V>`).
fn type_arguments<'t>(generic: &Node<'t>) -> Option<Node<'t>> {
    if generic.kind() != "generic_type" {
        return None;
    }
    named_children(generic).into_iter().find(|c| c.kind() == "type_arguments")
}

fn normalized(node: &Node, text: &str) -> String {
    source(node, text).split_whitespace().collect()
}

/// The declared type of a field of the class enclosing `node`, if the class declares it.
fn field_type<'t>(node: &Node<'t>, name: &str, text: &str) -> Option<Node<'t>> {
    let body = std::iter::successors(node.parent(), |n| n.parent())
        .find(|n| matches!(n.kind(), "class_body" | "enum_body_declarations"))?;
    named_children(&body).into_iter().filter(|m| m.kind() == "field_declaration").find_map(|field| {
        field
            .children_by_field_name("declarator", &mut field.walk())
            .any(|d| {
                d.child_by_field_name("dimensions").is_none()
                    && d.child_by_field_name("name").is_some_and(|n| source(&n, text) == name)
            })
            .then(|| field.child_by_field_name("type"))
            .flatten()
    })
}

/// The return type of the method that a `return` statement returns from, unless it is in a lambda.
fn return_type<'t>(statement: &Node<'t>) -> Option<Node<'t>> {
    let method = std::iter::successors(statement.parent(), |n| n.parent()).find(|n| {
        matches!(n.kind(), "method_declaration" | "lambda_expression" | "class_body" | "constructor_declaration")
    })?;
    (method.kind() == "method_declaration" && method.child_by_field_name("dimensions").is_none())
        .then(|| method.child_by_field_name("type"))
        .flatten()
}

/// Generic classes of `java.util` and `java.util.concurrent` that are often created with raw types.
const GENERIC_CLASSES: &[(&str, &str)] = &[
    ("java.util", "ArrayList"),
    ("java.util", "LinkedList"),
    ("java.util", "Vector"),
    ("java.util", "Stack"),
    ("java.util", "ArrayDeque"),
    ("java.util", "PriorityQueue"),
    ("java.util", "HashMap"),
    ("java.util", "LinkedHashMap"),
    ("java.util", "TreeMap"),
    ("java.util", "Hashtable"),
    ("java.util", "IdentityHashMap"),
    ("java.util", "WeakHashMap"),
    ("java.util", "HashSet"),
    ("java.util", "LinkedHashSet"),
    ("java.util", "TreeSet"),
    ("java.util.concurrent", "ConcurrentHashMap"),
    ("java.util.concurrent", "ConcurrentSkipListMap"),
    ("java.util.concurrent", "ConcurrentSkipListSet"),
    ("java.util.concurrent", "ConcurrentLinkedQueue"),
    ("java.util.concurrent", "ConcurrentLinkedDeque"),
    ("java.util.concurrent", "CopyOnWriteArrayList"),
    ("java.util.concurrent", "CopyOnWriteArraySet"),
    ("java.util.concurrent", "LinkedBlockingQueue"),
    ("java.util.concurrent", "LinkedBlockingDeque"),
    ("java.util.concurrent", "ArrayBlockingQueue"),
    ("java.util.concurrent", "PriorityBlockingQueue"),
    ("java.lang", "ThreadLocal"),
];

/// Whether the raw type is one of [`GENERIC_CLASSES`]: written fully qualified, or imported (by name or on demand)
/// or from `java.lang`, and not declared in the file.
fn is_known_generic(created: &Node, text: &str) -> bool {
    let written = source(created, text);
    let Some((package, name)) = GENERIC_CLASSES.iter().find(|(package, name)| {
        *name == written || written.strip_prefix(package).and_then(|w| w.strip_prefix('.')) == Some(name)
    }) else {
        return false;
    };
    if written != *name {
        return true;
    }
    let root = std::iter::successors(Some(*created), |n| n.parent()).last().unwrap_or(*created);
    let imported = *package == "java.lang"
        || named_children(&root).iter().filter(|i| i.kind() == "import_declaration").any(|import| {
            let import: String = source(import, text).split_whitespace().collect();
            import == format!("import{package}.{name};") || import == format!("import{package}.*;")
        });
    imported && !declares_type(text, name)
}

/// Whether the text may declare a type of that name (also in comments).
fn declares_type(text: &str, name: &str) -> bool {
    ["class ", "interface ", "enum ", "record "].iter().any(|keyword| {
        let declaration = format!("{keyword}{name}");
        text.match_indices(&declaration).any(|(i, _)| {
            !text[i + declaration.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '$')
        })
    })
}
