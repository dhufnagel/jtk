//! Helpers shared by the logging rules (and the security rules): resolving type names through the file's imports,
//! finding the declarations of names, the logging APIs, and what is locally known about the type of an expression.

use crate::lint::tree::{Context, for_each_descendant, strip_parens};
use jtk_parser::ast::{ClassDecl, ClassKind, K, LitKind, Mods, Tree, VarDecl};

/// The qualified name of a type or package (or annotation) without type arguments and annotations: `a.b.C<D>` is
/// `a.b.C`.
pub(in crate::lint) fn qualified_name(tree: &Tree) -> String {
    match &tree.kind {
        K::Ident(name) | K::Primitive(name) => name.clone(),
        K::MemberSelect { expr, name } => format!("{}.{name}", qualified_name(expr)),
        K::Parameterized { base, .. } | K::Annotated { base, .. } => qualified_name(base),
        _ => String::new(),
    }
}

/// The imports of the compilation unit.
pub(in crate::lint) fn imports(unit: &Tree) -> &[Tree] {
    match &unit.kind {
        K::CompilationUnit { imports, .. } => imports,
        _ => &[],
    }
}

/// The name a non-static import imports, and whether it is on demand: `import a.b.*;` imports `a.b` on demand.
pub(in crate::lint) fn type_import(import: &Tree) -> Option<(String, bool)> {
    let K::Import { is_static: false, name } = &import.kind else { return None };
    match &name.kind {
        K::MemberSelect { expr, name } if name == "*" => Some((qualified_name(expr), true)),
        _ => Some((qualified_name(name), false)),
    }
}

/// Whether the file declares a class or type parameter named `simple` anywhere.
pub(in crate::lint) fn declares_type(unit: &Tree, simple: &str) -> bool {
    let mut found = false;
    for_each_descendant(unit, &mut |tree, _| {
        found |= match &tree.kind {
            K::Class(class) => class.name == simple,
            K::TypeParameter { name, .. } => name == simple,
            _ => false,
        };
        !found
    });
    found
}

/// Whether the simple name `simple` refers to the type `qualified` through the file's imports: it is imported by a
/// single-type import, or by an on-demand import of its package while no other type of that name is imported or
/// declared in the file.
pub(in crate::lint) fn imports_type(unit: &Tree, simple: &str, qualified: &str) -> bool {
    let Some((package, last)) = qualified.rsplit_once('.') else { return false };
    if last != simple {
        return false;
    }
    let mut single = None;
    let mut on_demand = false;
    for (name, is_on_demand) in imports(unit).iter().filter_map(type_import) {
        if is_on_demand {
            on_demand |= name == package;
        } else if name.rsplit('.').next() == Some(simple) {
            single = Some(name);
        }
    }
    match single {
        Some(name) => name == qualified,
        None => on_demand && !declares_type(unit, simple),
    }
}

/// Whether the type or qualifier `tree` names the type `qualified`, by its qualified name or by an imported simple
/// name.
pub(in crate::lint) fn names_type(tree: &Tree, qualified: &str, unit: &Tree) -> bool {
    match &tree.kind {
        K::Ident(simple) => imports_type(unit, simple, qualified),
        K::MemberSelect { .. } => qualified_name(tree) == qualified,
        K::Parameterized { base, .. } | K::Annotated { base, .. } => names_type(base, qualified, unit),
        _ => false,
    }
}

/// Whether the modifiers have an annotation naming the type `qualified`.
pub(in crate::lint) fn has_annotation(mods: &Mods, qualified: &str, unit: &Tree) -> bool {
    mods.annotations.iter().any(|a| matches!(&a.kind, K::Annotation { ty, .. } if names_type(ty, qualified, unit)))
}

/// A logging API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::lint) enum Api {
    Slf4j,
    Log4j2,
    /// `java.util.logging`.
    Jul,
}

impl Api {
    /// The levels of the logging methods named after them, from the least to the most severe.
    pub(super) fn levels(self) -> &'static [&'static str] {
        match self {
            Api::Slf4j => &["trace", "debug", "info", "warn", "error"],
            Api::Log4j2 => &["trace", "debug", "info", "warn", "error", "fatal"],
            Api::Jul => &["finest", "finer", "fine", "config", "info", "warning", "severe"],
        }
    }
}

/// The logger types of the APIs.
const LOGGER_TYPES: &[(&str, Api)] = &[
    ("org.slf4j.Logger", Api::Slf4j),
    ("org.apache.logging.log4j.Logger", Api::Log4j2),
    ("java.util.logging.Logger", Api::Jul),
];

/// The Lombok annotations that generate a field `log`.
const LOMBOK_LOGGERS: &[(&str, Api)] = &[
    ("lombok.extern.slf4j.Slf4j", Api::Slf4j),
    ("lombok.extern.log4j.Log4j2", Api::Log4j2),
    ("lombok.extern.java.Log", Api::Jul),
];

/// The declaration a name refers to.
pub(in crate::lint) enum Declared<'t> {
    /// A local variable, parameter or field.
    Var(&'t VarDecl),
    /// The parameter of a `catch`: a throwable.
    CatchParameter,
    /// A logger field Lombok generates.
    Lombok(Api),
}

/// The declaration of the local variable, parameter or field `name` visible at `tree`, below `ancestors` (outermost
/// first). `None` if no declaration is found in the enclosing code, if the name is declared with extra array
/// dimensions (`int a[]`) or as a variable arity parameter, and if it may be a field inherited by an enclosing class
/// (which has a supertype and does not declare the name itself).
pub(in crate::lint) fn declaration<'t>(
    tree: &'t Tree,
    ancestors: &[&'t Tree],
    name: &str,
    context: &Context,
) -> Option<Declared<'t>> {
    let unit = ancestors.first()?;
    let mut child = tree;
    for &parent in ancestors.iter().rev() {
        if let Some(found) = declaration_in(parent, child, name, unit, context) {
            return found;
        }
        child = parent;
    }
    None
}

/// The declaration of `name` that `parent` makes visible to its child `child`: `Some(None)` if the declaration is
/// not usable, or if the name may be inherited, so that the search stops there.
fn declaration_in<'t>(
    parent: &'t Tree,
    child: &Tree,
    name: &str,
    unit: &Tree,
    context: &Context,
) -> Option<Option<Declared<'t>>> {
    let before = |s: &&Tree| s.end <= child.start;
    let is_child = |t: &Tree| std::ptr::eq(t, child);
    let var = |v: &'t VarDecl| usable(v).then_some(Declared::Var(v));
    match &parent.kind {
        K::Block { stmts, .. } => stmts.iter().filter(before).filter_map(|s| declared_in(s, name)).next_back().map(var),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            cases.iter().flat_map(|c| &c.stmts).filter(before).filter_map(|s| declared_in(s, name)).next_back().map(var)
        }
        K::For { init, .. } => init.iter().find_map(|s| declared_in(s, name)).map(var),
        K::ForEach { var: v, .. } if v.name == name => Some(var(v)),
        K::Try { resources, catches, .. } => {
            let resource = |r: &'t Tree| match &r.kind {
                K::Var(v) if v.name == name => Some(var(v)),
                _ => None,
            };
            let catch = catches.iter().find(|c| c.start <= child.start && child.end <= c.block.end);
            if catch.is_some_and(|c| c.param.name == name) {
                return Some(Some(Declared::CatchParameter));
            }
            let earlier =
                if resources.iter().any(is_child) { resources.iter().filter(before).find_map(resource) } else { None };
            earlier.or_else(|| resources.iter().find_map(resource))
        }
        K::Method(method) => method.params.iter().find(|p| p.name == name).map(|p| parameter(p, context)),
        K::Lambda { params, .. } => params.iter().find(|p| p.name == name).map(|p| parameter(p, context)),
        K::Class(class) => {
            let constant_body =
                class.enum_constants.iter().find_map(|c| c.body.as_deref().filter(|b| b.iter().any(is_child)));
            let body: &[Tree] = match constant_body {
                Some(body) => body,
                None if class.kind != ClassKind::Annotation && class.members.iter().any(is_child) => &class.members,
                None => &[],
            };
            let components = if class.kind == ClassKind::Record { &class.record_components[..] } else { &[] };
            let found = body
                .iter()
                .find_map(|m| declared_in(m, name))
                .map(var)
                .or_else(|| components.iter().find(|p| p.name == name).map(|p| parameter(p, context)));
            let lombok = || {
                let api =
                    LOMBOK_LOGGERS.iter().find(|(a, _)| has_annotation(&class.mods, a, unit)).map(|(_, api)| *api);
                api.filter(|_| name == "log" && constant_body.is_none()).map(|api| Some(Declared::Lombok(api)))
            };
            found.or_else(lombok).or(may_inherit(class).then_some(None))
        }
        // An anonymous class may inherit the name.
        K::NewClass { body: Some(body), .. } if body.iter().any(is_child) => {
            Some(body.iter().find_map(|m| declared_in(m, name)).and_then(var))
        }
        _ => None,
    }
}

fn may_inherit(class: &ClassDecl) -> bool {
    match class.kind {
        ClassKind::Class | ClassKind::Enum | ClassKind::Record => {
            class.extends.is_some() || !class.implements.is_empty()
        }
        ClassKind::Interface => !class.implements.is_empty(),
        ClassKind::Annotation => false,
    }
}

/// The variable of a local variable or field declaration that declares `name`.
fn declared_in<'t>(declaration: &'t Tree, name: &str) -> Option<&'t VarDecl> {
    let K::Vars(vars) = &declaration.kind else { return None };
    vars.iter().find(|v| v.name == name)
}

/// Whether the declared type of a variable is its type: it has no extra array dimensions after the name.
fn usable(var: &VarDecl) -> bool {
    var.ty.as_deref().is_none_or(|ty| ty.end <= var.name_pos)
}

/// A parameter, unless it is a variable arity one.
fn parameter<'t>(param: &'t VarDecl, context: &Context) -> Option<Declared<'t>> {
    let is_variable_arity = context.tokens().previous(param.name_pos).is_some_and(|t| &context.text[t] == "...");
    (usable(param) && !is_variable_arity).then_some(Declared::Var(param))
}

/// The logging API of the logger `receiver` (an identifier) of a call below `ancestors`: the declared type of the
/// variable, or the Lombok annotation that generates it. Log4j 2 loggers count only when created with
/// `LogManager.getLogger`, as the formatter loggers use `%s` instead of `{}`.
pub(super) fn logger_api(receiver: &Tree, ancestors: &[&Tree], context: &Context) -> Option<Api> {
    let K::Ident(name) = &receiver.kind else { return None };
    let unit = ancestors.first()?;
    match declaration(receiver, ancestors, name, context)? {
        Declared::Lombok(api) => Some(api),
        Declared::CatchParameter => None,
        Declared::Var(var) => {
            let ty = var.ty.as_deref()?;
            let mut apis = LOGGER_TYPES.iter().filter(|(t, _)| names_type(ty, t, unit)).map(|(_, api)| *api);
            let api = apis.next().filter(|_| apis.next().is_none())?;
            let created_by_get_logger = var.init.as_deref().is_some_and(|init| {
                matches!(&init.kind, K::MethodInvocation { select, .. }
                    if matches!(&select.kind, K::MemberSelect { expr, name }
                        if name == "getLogger" && names_type(expr, "org.apache.logging.log4j.LogManager", unit)))
            });
            (api != Api::Log4j2 || created_by_get_logger).then_some(api)
        }
    }
}

/// A call of a logging method named after a level, such as `log.info(...)`, on a logger of a known API.
pub(super) struct LogCall<'t> {
    pub api: Api,
    /// The logger variable.
    pub receiver: &'t str,
    /// The index of the level in [`Api::levels`].
    pub level: usize,
    pub args: &'t [Tree],
}

pub(super) fn log_call<'t>(tree: &'t Tree, ancestors: &[&'t Tree], context: &Context) -> Option<LogCall<'t>> {
    let K::MethodInvocation { select, args, .. } = &tree.kind else { return None };
    let K::MemberSelect { expr: receiver, name } = &select.kind else { return None };
    let K::Ident(receiver_name) = &receiver.kind else { return None };
    let api = logger_api(receiver, ancestors, context)?;
    let level = api.levels().iter().position(|l| l == name)?;
    Some(LogCall { api, receiver: receiver_name, level, args })
}

/// What is known locally about whether an expression is a throwable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Throwable,
    /// Not a throwable; whether it is an array.
    Other {
        array: bool,
    },
    Unknown,
}

/// What is known about the expression `tree` (below `ancestors`) without types: catch parameters, `new XException()`
/// and variables declared with a type named `Throwable` or `...Exception` are throwables; literals, operators,
/// arrays, lambdas, `toString()`, and variables of primitive types, arrays, `String` and the boxed types are not.
pub(super) fn kind(tree: &Tree, ancestors: &[&Tree], context: &Context) -> Kind {
    let Some(unit) = ancestors.first() else { return Kind::Unknown };
    let expression = strip_parens(tree);
    match &expression.kind {
        K::Literal(_)
        | K::Binary { .. }
        | K::Unary { .. }
        | K::InstanceOf { .. }
        | K::Lambda { .. }
        | K::MemberRef { .. } => Kind::Other { array: false },
        K::MemberSelect { name, .. } if name == "class" => Kind::Other { array: false },
        K::NewArray { .. } => Kind::Other { array: true },
        K::NewClass { ident, body: None, .. } => type_kind(ident, unit),
        K::Cast { ty, .. } => type_kind(ty, unit),
        K::MethodInvocation { select, args, .. } => match &select.kind {
            K::MemberSelect { name, .. } if args.is_empty() && matches!(name.as_str(), "toString" | "hashCode") => {
                Kind::Other { array: false }
            }
            _ => Kind::Unknown,
        },
        K::Conditional { then, else_, .. } => {
            let mut inner = ancestors.to_vec();
            inner.push(expression);
            match (kind(then, &inner, context), kind(else_, &inner, context)) {
                (a, b) if a == b => a,
                (Kind::Other { .. }, Kind::Other { .. }) => Kind::Other { array: true },
                _ => Kind::Unknown,
            }
        }
        K::Ident(name) => {
            // The parentheses around the name do not matter for finding its declaration.
            match declaration(tree, ancestors, name, context) {
                Some(Declared::CatchParameter) => Kind::Throwable,
                Some(Declared::Var(var)) => var.ty.as_deref().map_or(Kind::Unknown, |ty| type_kind(ty, unit)),
                _ => Kind::Unknown,
            }
        }
        _ => Kind::Unknown,
    }
}

/// What the type `ty` tells about its values (see [`kind`]).
fn type_kind(ty: &Tree, unit: &Tree) -> Kind {
    const FINAL_TYPES: &[&str] =
        &["String", "Integer", "Long", "Short", "Byte", "Character", "Boolean", "Double", "Float", "StringBuilder"];
    match &ty.kind {
        K::Primitive(_) => Kind::Other { array: false },
        K::ArrayType { .. } => Kind::Other { array: true },
        K::Parameterized { base, .. } | K::Annotated { base, .. } => type_kind(base, unit),
        K::Ident(name) | K::MemberSelect { name, .. } => {
            let java_lang = |simple: &str| match &ty.kind {
                K::Ident(_) => !declares_type(unit, simple) && !imports_simple_name(unit, simple),
                _ => qualified_name(ty) == format!("java.lang.{simple}"),
            };
            if (name == "Throwable" || name.ends_with("Exception")) && !declares_type(unit, name) {
                Kind::Throwable
            } else if FINAL_TYPES.contains(&name.as_str()) && java_lang(name) {
                Kind::Other { array: false }
            } else {
                Kind::Unknown
            }
        }
        _ => Kind::Unknown,
    }
}

/// Whether a single-type import imports a type named `simple`.
fn imports_simple_name(unit: &Tree, simple: &str) -> bool {
    imports(unit)
        .iter()
        .filter_map(type_import)
        .any(|(name, on_demand)| !on_demand && name.rsplit('.').next() == Some(simple))
}

/// The value of a string literal, or of a concatenation of string literals, if it has no escape sequences that
/// could stand for `{`, `}` or `\` (Unicode and octal escapes): other escapes become a space.
pub(in crate::lint) fn constant_string(tree: &Tree, text: &str) -> Option<String> {
    match &strip_parens(tree).kind {
        K::Literal(LitKind::String) => {
            let literal = strip_parens(tree);
            let content = text[literal.start..literal.end].strip_prefix('"')?.strip_suffix('"')?;
            if content.starts_with("\"\"") {
                return None;
            }
            let mut value = String::new();
            let mut chars = content.chars();
            while let Some(c) = chars.next() {
                if c != '\\' {
                    value.push(c);
                    continue;
                }
                match chars.next()? {
                    '\\' => value.push('\\'),
                    'b' | 't' | 'n' | 'f' | 'r' | 's' | '"' | '\'' => value.push(' '),
                    _ => return None,
                }
            }
            Some(value)
        }
        K::Binary { op, lhs, rhs } if op == "+" => Some(constant_string(lhs, text)? + &constant_string(rhs, text)?),
        _ => None,
    }
}

/// The number of `{}` in a message that are not escaped by a backslash; `None` if one follows two backslashes.
pub(super) fn count_placeholders(message: &str) -> Option<usize> {
    let bytes = message.as_bytes();
    let mut count = 0;
    for (i, _) in message.match_indices("{}") {
        let backslashes = bytes[..i].iter().rev().take_while(|&&b| b == b'\\').count();
        match backslashes {
            0 => count += 1,
            1 => {}
            _ => return None,
        }
    }
    Some(count)
}
