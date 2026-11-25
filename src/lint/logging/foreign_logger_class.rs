//! L002: a logger field created for another class than the one declaring it, such as
//! `class A { Logger log = LoggerFactory.getLogger(B.class); }` (IntelliJ LoggerInitializedWithForeignClass, Baseline
//! LoggerEnclosingClass): mostly copied code, and the log messages then appear under the wrong name.
//!
//! Checks the initializers of fields that call SLF4J's `LoggerFactory.getLogger`, Log4j 2's `LogManager.getLogger`
//! or `java.util.logging.Logger.getLogger` with `B.class` or `B.class.getName()`, the factory resolved through the
//! file's imports. Safe fix, as in Baseline: names the declaring class instead. Like Baseline, only fields count (a
//! method may well want the logger of another class, as tests that capture its output do), and only a simple class
//! name; fields of anonymous classes and enum constant bodies are left out.

use crate::lint::logging::shared::names_type;
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

/// The classes whose static `getLogger` creates a logger.
const FACTORIES: &[&str] =
    &["org.slf4j.LoggerFactory", "org.apache.logging.log4j.LogManager", "java.util.logging.Logger"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Vars(vars) = &tree.kind else { return };
    let Some(K::Class(class)) = context.parent().map(|p| &p.kind) else { return };
    let Some(unit) = context.ancestors.first() else { return };
    if !class.members.iter().any(|m| std::ptr::eq(m, tree)) {
        return;
    }
    for init in vars.iter().filter_map(|v| v.init.as_deref()) {
        let K::MethodInvocation { select, args, .. } = &init.kind else { continue };
        let K::MemberSelect { expr: factory, name } = &select.kind else { continue };
        let [argument] = args.as_slice() else { continue };
        if name != "getLogger" || !FACTORIES.iter().any(|f| names_type(factory, f, unit)) {
            continue;
        }
        let Some(class_name) = class_literal_name(argument) else { continue };
        let K::Ident(named) = &class_name.kind else { continue };
        if *named != class.name {
            let message = format!("logger created for {named} instead of {}", class.name);
            let fix = vec![Edit::new(class_name.start..class_name.end, class.name.clone())];
            diagnostics.push(Diagnostic::new(Rule::ForeignLoggerClass, class_name.start, message).with_fix(fix));
        }
    }
}

/// The class `C` of `C.class` or `C.class.getName()`.
fn class_literal_name(argument: &Tree) -> Option<&Tree> {
    let literal = match &argument.kind {
        K::MethodInvocation { select, args, .. } if args.is_empty() => match &select.kind {
            K::MemberSelect { expr, name } if name == "getName" => expr,
            _ => return None,
        },
        _ => argument,
    };
    match &literal.kind {
        K::MemberSelect { expr, name } if name == "class" => Some(expr),
        _ => None,
    }
}
