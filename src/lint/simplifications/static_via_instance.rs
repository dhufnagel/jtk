//! S047: a static field or method accessed through a variable instead of its class (javac `static`, Eclipse and
//! NetBeans static access via reference, Error Prone StaticQualifiedUsingExpression, IntelliJ
//! AccessStaticViaInstance): `thread.sleep(10)` reads as if it acted on `thread`, but sleeps the current thread.
//!
//! Without type information, only a variable, parameter or field declared in the file with a class type counts as
//! receiver, and only if the member is certainly static there: a class declared in the file whose own static field
//! or enum constant it is, or whose methods of that name are all static (and which has no supertype that could add
//! an instance method of the name), or a member from a short list of static members of `java.lang` classes
//! (`Thread.sleep`, `String.valueOf`, `Integer.MAX_VALUE`, ...). Members such as `Integer.toString`, which has
//! static and instance overloads, are not in it.
//!
//! The fix writes the declared class instead of the variable. Evaluating a variable has no side effects and does not
//! throw for a static member even if it is null, so the fix is safe.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{ClassDecl, ClassKind, K, Modifier, Tree};

use super::shared_batch3::{class_name, declared_type, declares_locally, imports, member_classes, refers_to};

/// Static members of `java.lang` classes that have no instance member of the same name: the class, its static
/// methods and its static fields.
const JAVA_LANG: &[(&str, &[&str], &[&str])] = &[
    (
        "Thread",
        &["sleep", "currentThread", "yield", "interrupted", "onSpinWait", "holdsLock", "dumpStack", "activeCount"],
        &["MIN_PRIORITY", "NORM_PRIORITY", "MAX_PRIORITY"],
    ),
    ("String", &["valueOf", "format", "join", "copyValueOf"], &["CASE_INSENSITIVE_ORDER"]),
    ("Integer", &["valueOf", "parseInt", "max", "min", "sum"], &["MAX_VALUE", "MIN_VALUE", "SIZE", "BYTES"]),
    ("Long", &["valueOf", "parseLong", "max", "min", "sum"], &["MAX_VALUE", "MIN_VALUE", "SIZE", "BYTES"]),
    ("Boolean", &["valueOf", "parseBoolean", "logicalAnd", "logicalOr", "logicalXor"], &["TRUE", "FALSE"]),
    ("Double", &["valueOf", "parseDouble", "max", "min", "sum"], &["MAX_VALUE", "MIN_VALUE", "NaN"]),
    ("Character", &["isDigit", "isLetter", "isLetterOrDigit", "isWhitespace", "isUpperCase", "isLowerCase"], &[]),
];

/// The methods every enum or record inherits, which could be instance overloads of a static method of the name.
const INHERITED: &[&str] = &[
    "equals",
    "hashCode",
    "toString",
    "getClass",
    "notify",
    "notifyAll",
    "wait",
    "name",
    "ordinal",
    "compareTo",
    "getDeclaringClass",
    "describeConstable",
    "valueOf",
];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MemberSelect { expr: receiver, name: member } = &tree.kind else { return };
    let K::Ident(variable) = &receiver.kind else { return };
    if variable == "this" || variable == "super" || member == "class" {
        return;
    }
    let is_call = matches!(context.parent().map(|p| &p.kind), Some(K::MethodInvocation { select, .. }) if std::ptr::eq(&**select, tree));
    let Some(ty) = declared_type(tree, &context.ancestors, variable, context) else { return };
    let Some(class) = class_name(ty, context) else { return };
    if &class == variable || !is_static(&class, member, is_call, context) {
        return;
    }
    let message = format!("Static member '{member}' accessed via an instance; use '{class}'");
    let diagnostic = Diagnostic::new(Rule::StaticViaInstance, receiver.start, message);
    diagnostics.push(diagnostic.with_fix(vec![Edit::new(receiver.start..receiver.end, class)]));
}

/// Whether the member of the class written as `class` is certainly static.
fn is_static(class: &str, member: &str, is_call: bool, context: &Context) -> bool {
    let ancestors = &context.ancestors;
    let known = JAVA_LANG.iter().find(|(name, methods, fields)| {
        let members = if is_call { methods } else { fields };
        members.contains(&member) && refers_to(class, &format!("java.lang.{name}"), ancestors, context)
    });
    if known.is_some() {
        return true;
    }
    if class.contains('.') || declares_locally(ancestors, class) || imports(ancestors, class) {
        return false;
    }
    let Some(unit) = ancestors.first() else { return false };
    match member_classes(unit, class).as_slice() {
        [declared] => {
            if is_call {
                has_only_static_methods(declared, member)
            } else {
                has_static_field(declared, member)
            }
        }
        _ => false,
    }
}

fn has_static_field(class: &ClassDecl, field: &str) -> bool {
    let interface = matches!(class.kind, ClassKind::Interface | ClassKind::Annotation);
    let constant = class.enum_constants.iter().any(|c| c.name == field);
    constant
        || class.members.iter().any(|m| match &m.kind {
            K::Vars(vars) => vars.iter().any(|v| v.name == field && (interface || v.mods.has(Modifier::Static))),
            _ => false,
        })
}

/// Whether the class declares methods named `method`, all static, and cannot inherit an instance method of the name.
fn has_only_static_methods(class: &ClassDecl, method: &str) -> bool {
    let may_inherit = match class.kind {
        ClassKind::Class => class.extends.is_some() || !class.implements.is_empty() || INHERITED[..7].contains(&method),
        ClassKind::Enum | ClassKind::Record => !class.implements.is_empty() || INHERITED.contains(&method),
        // Static methods of interfaces cannot be called through an instance.
        ClassKind::Interface | ClassKind::Annotation => true,
    };
    let methods: Vec<_> = class
        .members
        .iter()
        .filter_map(|m| match &m.kind {
            K::Method(m) if m.name == method => Some(m),
            _ => None,
        })
        .collect();
    !may_inherit && !methods.is_empty() && methods.iter().all(|m| m.mods.has(Modifier::Static))
}
