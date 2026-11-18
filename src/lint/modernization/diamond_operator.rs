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

use std::ops::Range;

use crate::ast::{ClassKind, K, Tree};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};

use super::shared_b::declared_type;

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    match &tree.kind {
        K::Vars(vars) => {
            for var in vars {
                if let (Some(value), Some(target)) = (&var.init, &var.ty)
                    && target.end <= var.name_pos
                {
                    check_creation(value, target, context, diagnostics);
                }
            }
        }
        K::Assign { op, lhs, rhs } if op == "=" && matches!(rhs.kind, K::NewClass { .. }) => {
            let target = match &lhs.kind {
                K::Ident(name) => declared_type(tree, &context.ancestors, name, context),
                K::MemberSelect { expr, name } if matches!(&expr.kind, K::Ident(this) if this == "this") => {
                    field_type(tree, &context.ancestors, name)
                }
                _ => None,
            };
            if let Some(target) = target {
                check_creation(rhs, target, context, diagnostics);
            }
        }
        K::Return(Some(value)) if matches!(value.kind, K::NewClass { .. }) => {
            if let Some(target) = return_type(tree, &context.ancestors) {
                check_creation(value, target, context, diagnostics);
            }
        }
        _ => {}
    }
}

fn check_creation(value: &Tree, target: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::NewClass { type_args, ident, args, body: None, .. } = &value.kind else { return };
    if !type_args.is_empty() {
        return;
    }
    let created = match &ident.kind {
        K::Annotated { base, .. } => base,
        _ => ident,
    };
    let has_arguments = !args.is_empty();
    let target_arguments = type_arguments(target, context);
    if let K::Parameterized { args: created_arguments, .. } = &created.kind {
        let Some(arguments) = type_arguments(created, context).filter(|_| !created_arguments.is_empty()) else {
            return;
        };
        let Some(target_arguments) = target_arguments else { return };
        let target_arguments = normalized(&context.text[target_arguments]);
        if has_arguments
            && (normalized(&context.text[arguments.clone()]) != target_arguments || target_arguments.contains('?'))
        {
            return;
        }
        let message = "Type arguments can be inferred: use <>";
        let diagnostic = Diagnostic::new(Rule::DiamondOperator, arguments.start, message);
        diagnostics.push(if context.tokens().has_comment(arguments.clone()) {
            diagnostic
        } else {
            diagnostic.with_fix(vec![Edit::new(arguments, "<>")])
        });
    } else if (!has_arguments || target_arguments.is_none()) && is_known_generic(created, context) {
        let fix = Edit::new(created.end..created.end, "<>");
        let message = "Raw type can be avoided: use <>";
        diagnostics.push(Diagnostic::new(Rule::DiamondOperator, created.start, message).with_fix(vec![fix]));
    }
}

/// The range of the type arguments of a generic type (`List<String>`, `Map.Entry<K, V>`), from `<` to `>`.
fn type_arguments(generic: &Tree, context: &Context) -> Option<Range<usize>> {
    let K::Parameterized { base, .. } = &generic.kind else { return None };
    let open = context.tokens().next(base.end)?;
    Some(open.start..generic.end)
}

fn normalized(code: &str) -> String {
    code.split_whitespace().collect()
}

/// The declared type of the field `name` of the class body enclosing `tree`, if the body declares it.
fn field_type<'t>(tree: &'t Tree, ancestors: &[&'t Tree], name: &str) -> Option<&'t Tree> {
    let mut child = tree;
    let body = ancestors.iter().rev().find_map(|&parent| {
        let is_child = |t: &Tree| std::ptr::eq(t, child);
        let body = match &parent.kind {
            K::Class(class) if !matches!(class.kind, ClassKind::Interface | ClassKind::Annotation) => {
                let constant_body =
                    class.enum_constants.iter().find_map(|c| c.body.as_deref().filter(|b| b.iter().any(is_child)));
                constant_body.or_else(|| class.members.iter().any(is_child).then_some(&class.members[..]))
            }
            K::NewClass { body: Some(body), .. } if body.iter().any(is_child) => Some(&body[..]),
            _ => None,
        };
        child = parent;
        body
    })?;
    body.iter()
        .find_map(|member| match &member.kind {
            K::Vars(vars) => vars.iter().find(|v| v.name == name && v.ty.as_ref().is_some_and(|t| t.end <= v.name_pos)),
            _ => None,
        })?
        .ty
        .as_deref()
}

/// The return type of the method that a `return` statement returns from, unless it is in a lambda.
fn return_type<'t>(statement: &'t Tree, ancestors: &[&'t Tree]) -> Option<&'t Tree> {
    let mut child = statement;
    for &parent in ancestors.iter().rev() {
        match &parent.kind {
            K::Method(method) => return method.ret.as_deref().filter(|r| r.end <= method.name_pos),
            K::Lambda { .. } | K::Class(_) => return None,
            K::NewClass { body: Some(body), .. } if body.iter().any(|m| std::ptr::eq(m, child)) => return None,
            _ => child = parent,
        }
    }
    None
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
fn is_known_generic(created: &Tree, context: &Context) -> bool {
    let written = context.source(created);
    let Some((package, name)) = GENERIC_CLASSES.iter().find(|(package, name)| {
        *name == written || written.strip_prefix(package).and_then(|w| w.strip_prefix('.')) == Some(name)
    }) else {
        return false;
    };
    if written != *name {
        return true;
    }
    let K::CompilationUnit { imports, .. } = &context.ancestors[0].kind else { return false };
    let imported = *package == "java.lang"
        || imports.iter().any(|import| {
            let K::Import { is_static: false, name: imported } = &import.kind else { return false };
            let imported = normalized(context.source(imported));
            imported == format!("{package}.{name}") || imported == format!("{package}.*")
        });
    imported && !declares_type(context.text, name)
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
