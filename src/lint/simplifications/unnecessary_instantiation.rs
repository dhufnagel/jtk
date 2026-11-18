//! S019: instances of `String` and the primitive wrappers created with `new` (PMD StringInstantiation,
//! PrimitiveWrapperInstantiation): `new String("x")` is `"x"`, `new Integer(5)` is better `Integer.valueOf(5)`,
//! which may reuse cached instances, and `new Boolean(true)` or `Boolean.valueOf(true)` is `Boolean.TRUE`.
//!
//! Like the replacements PMD suggests, the fixes give up object identity: code comparing the instances with `==` or
//! synchronizing on them behaves differently. Without type resolution, `new String(x)` is only reported if `x` is a
//! string literal, a concatenation with one, or a variable declared as `String` (a `char[]` or `byte[]` copy is
//! fine), and only the literal is fixed: `new String(null)` throws. `new Float(x)` is only fixed if `x` cannot be a
//! `double`, which `Float.valueOf` does not accept.

use crate::ast::{ClassKind, K, LitKind, Tree, VarDecl};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};

const WRAPPERS: &[&str] = &["Integer", "Long", "Short", "Byte", "Character", "Double", "Float"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::NewClass { outer, type_args, ident: created, args, body } = &tree.kind else {
        check_boolean_value_of(tree, context, diagnostics);
        return;
    };
    let text = context.text;
    // The annotations of `new @A Integer(1)` are not part of the type in tree-sitter's syntax tree.
    let unannotated = match &created.kind {
        K::Annotated { base, .. } => base,
        _ => created,
    };
    let written = context.source(unannotated);
    let simple = written.strip_prefix("java.lang.").unwrap_or(written);
    if !(simple == "String" || simple == "Boolean" || WRAPPERS.contains(&simple))
        || simple == written && is_shadowed(simple, text)
        || body.is_some()
        || !type_args.is_empty()
    {
        return;
    }
    let tokens = context.tokens();
    let keyword_and_type = tree.start..created.end;
    let can_rewrite = !tokens.has_comment(keyword_and_type.clone()) && outer.is_none();
    match simple {
        "String" => check_string(tree, args, context, diagnostics),
        "Boolean" => {
            let [argument] = &args[..] else { return };
            let (shown, constant) = literal_value(argument, context);
            let message =
                format!("Do not use `new Boolean({shown})`, prefer `{}`", constant.unwrap_or("Boolean.valueOf"));
            let diagnostic = Diagnostic::new(Rule::UnnecessaryInstantiation, tree.start, message);
            let fix = match constant {
                Some(constant) if !tokens.has_comment(tree.start..tree.end) => {
                    Some(Edit::new(tree.start..tree.end, format!("{}.{}", written, &constant["Boolean.".len()..])))
                }
                Some(_) => None,
                None => can_rewrite.then(|| Edit::new(keyword_and_type, format!("{written}.valueOf"))),
            };
            diagnostics.push(with_optional_fix(context, diagnostic, fix));
        }
        _ => {
            let message = format!("Do not use `new {simple}(...)`, prefer `{simple}.valueOf(...)`");
            let diagnostic = Diagnostic::new(Rule::UnnecessaryInstantiation, tree.start, message);
            let accepts = match &args[..] {
                [argument] => simple != "Float" || is_float_compatible(argument, context),
                _ => false,
            };
            let fix = (can_rewrite && accepts).then(|| Edit::new(keyword_and_type, format!("{written}.valueOf")));
            diagnostics.push(with_optional_fix(context, diagnostic, fix));
        }
    }
}

fn check_string(creation: &Tree, arguments: &[Tree], context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let message = "Avoid instantiating String objects; this is usually unnecessary";
    let diagnostic = Diagnostic::new(Rule::UnnecessaryInstantiation, creation.start, message);
    let fix = match arguments {
        [] => Some("\"\"".to_string()),
        [argument] if matches!(argument.kind, K::Literal(LitKind::String)) => {
            Some(context.source(argument).to_string())
        }
        [argument] if is_string(argument, creation, context) => None,
        _ => return,
    };
    let range = creation.start..creation.end;
    let fix = fix.filter(|_| !context.tokens().has_comment(range.clone())).map(|r| Edit::new(range.clone(), r));
    diagnostics.push(with_optional_fix(context, diagnostic, fix));
}

/// Whether the expression (in the argument of `creation`) is known to be a `String`.
fn is_string(expression: &Tree, creation: &Tree, context: &Context) -> bool {
    let expression = strip_parens(expression);
    match &expression.kind {
        K::Literal(LitKind::String) => true,
        K::Binary { op, lhs, rhs } if op == "+" => {
            is_string(lhs, creation, context) || is_string(rhs, creation, context)
        }
        K::Ident(name) => {
            declared_type(name, creation, context).is_some_and(|t| matches!(t, "String" | "java.lang.String"))
        }
        K::ArrayAccess { .. } => {
            let mut array = expression;
            let mut depth = 0;
            while let K::ArrayAccess { expr, .. } = &array.kind {
                array = strip_parens(expr);
                depth += 1;
            }
            let K::Ident(name) = &array.kind else { return false };
            declared_type(name, creation, context).is_some_and(|t| {
                let declared: String = t.split_whitespace().collect();
                declared.strip_prefix("String").is_some_and(|dims| dims == "[]".repeat(depth))
            })
        }
        K::MethodInvocation { select, .. } => match &select.kind {
            K::Ident(name) | K::MemberSelect { name, .. } => STRING_METHODS.contains(&name.as_str()),
            _ => false,
        },
        _ => false,
    }
}

/// Methods that return a `String` whatever their receiver (`toString`) or on the receivers they are usually called
/// on (`String`, `System`).
const STRING_METHODS: &[&str] = &[
    "toString",
    "substring",
    "trim",
    "strip",
    "toUpperCase",
    "toLowerCase",
    "concat",
    "replace",
    "replaceAll",
    "replaceFirst",
    "intern",
    "getProperty",
    "getenv",
];

/// The declared type of the local variable, parameter or field `name` visible at `creation`, as written. `None` if no
/// declaration is found in the enclosing code, if the name is declared with extra array dimensions (`int a[]`) or as
/// an implicitly typed lambda parameter, and if it may be a field inherited by an enclosing class (which has a
/// supertype and does not declare the name itself). `var` is the type of variables declared with it.
fn declared_type<'t>(name: &str, creation: &'t Tree, context: &Context<'t>) -> Option<&'t str> {
    let ancestors = &context.ancestors;
    for (index, parent) in ancestors.iter().enumerate().rev() {
        let child = ancestors.get(index + 1).copied().unwrap_or(creation);
        let is_child = |t: &Tree| std::ptr::eq(t, child);
        let before = |t: &&Tree| t.end <= child.start;
        let found = match &parent.kind {
            K::Block { stmts, .. } => {
                stmts.iter().filter(before).filter_map(|s| variable_type(s, name, context)).next_back()
            }
            K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => cases
                .iter()
                .flat_map(|c| &c.stmts)
                .filter(before)
                .filter_map(|s| variable_type(s, name, context))
                .next_back(),
            K::Class(class) => {
                let constant_body =
                    class.enum_constants.iter().filter_map(|c| c.body.as_deref()).find(|b| b.iter().any(is_child));
                let members = constant_body.unwrap_or(&class.members);
                let field = members.iter().find_map(|m| variable_type(m, name, context));
                field.or_else(|| parameter_type(&class.record_components, name, context))
            }
            K::NewClass { body: Some(members), .. } => members.iter().find_map(|m| variable_type(m, name, context)),
            K::For { init, .. } => init.iter().find_map(|i| variable_type(i, name, context)),
            K::ForEach { var, .. } if var.name == name => declared(var, context),
            K::Try { resources, catches, .. } => {
                let catch = catches.iter().find(|c| is_child(&c.block) && c.param.name == name);
                let resource_type = |r: &Tree| match &r.kind {
                    K::Var(var) if var.name == name => declared(var, context),
                    _ => None,
                };
                match catch {
                    Some(catch) => catch.param.ty.as_deref().map(|t| context.source(t)),
                    None => resources
                        .iter()
                        .filter(before)
                        .find_map(resource_type)
                        .or_else(|| resources.iter().find_map(resource_type)),
                }
            }
            K::Method(method) => parameter_type(&method.params, name, context),
            K::Lambda { params, .. } => {
                if params.iter().any(|p| p.ty.is_none() && p.name == name) {
                    return None;
                }
                parameter_type(params, name, context)
            }
            _ => None,
        };
        if found.is_some() {
            return found;
        }
        let may_inherit = match &parent.kind {
            K::Class(class) => match class.kind {
                ClassKind::Class | ClassKind::Enum | ClassKind::Record => {
                    class.extends.is_some() || !class.implements.is_empty()
                }
                ClassKind::Interface => !class.implements.is_empty(),
                ClassKind::Annotation => false,
            },
            K::NewClass { body: Some(members), .. } => members.iter().any(is_child),
            _ => false,
        };
        if may_inherit {
            return None;
        }
    }
    None
}

/// The written type of a variable without extra array dimensions, `var` if it has none.
fn declared<'t>(var: &VarDecl, context: &Context<'t>) -> Option<&'t str> {
    match var.ty.as_deref() {
        None => Some("var"),
        Some(ty) if ty.end > var.name_pos => None,
        Some(ty) => Some(context.source(ty)),
    }
}

/// The type of a local variable or field declaration that declares `name` without extra array dimensions.
fn variable_type<'t>(declaration: &Tree, name: &str, context: &Context<'t>) -> Option<&'t str> {
    let K::Vars(vars) = &declaration.kind else { return None };
    declared(vars.iter().find(|v| v.name == name)?, context)
}

/// The type of the parameter `name` (not a variable arity one) in a parameter list.
fn parameter_type<'t>(params: &[VarDecl], name: &str, context: &Context<'t>) -> Option<&'t str> {
    params.iter().filter(|p| p.name == name && p.ty.is_some()).find_map(|p| {
        let is_variable_arity = context.tokens().previous(p.name_pos).is_some_and(|t| &context.text[t] == "...");
        if is_variable_arity { None } else { declared(p, context) }
    })
}

/// Whether a class of the `java.lang` name may be imported or declared in the file, so that the simple name means
/// another class.
fn is_shadowed(name: &str, text: &str) -> bool {
    let imported = text.lines().any(|line| {
        let line = line.trim();
        line.starts_with("import ") && line.ends_with(&format!(".{name};")) && !line.contains("java.lang.")
    });
    imported
        || ["class ", "interface ", "enum ", "record "].iter().any(|keyword| {
            let declaration = format!("{keyword}{name}");
            text.match_indices(&declaration).any(|(i, _)| {
                !text[i + declaration.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '$')
            })
        })
}

/// `Boolean.valueOf(true)` and `Boolean.valueOf("false")`, whose value is a constant.
fn check_boolean_value_of(call: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { select, type_args, args } = &call.kind else { return };
    let K::MemberSelect { expr: object, name } = &select.kind else { return };
    let written = context.source(object);
    if name != "valueOf"
        || !(written == "java.lang.Boolean" || written == "Boolean" && !is_shadowed("Boolean", context.text))
        || !type_args.is_empty()
    {
        return;
    }
    let [argument] = &args[..] else { return };
    if !matches!(argument.kind, K::Literal(LitKind::String | LitKind::Boolean)) {
        return;
    }
    let (shown, constant) = literal_value(argument, context);
    let message = match constant {
        Some(constant) => format!("Do not use `Boolean.valueOf({shown})`, prefer `{constant}`"),
        None => format!("Do not use `Boolean.valueOf({shown})`, prefer `Boolean.valueOf`"),
    };
    let diagnostic = Diagnostic::new(Rule::UnnecessaryInstantiation, call.start, message);
    let range = call.start..call.end;
    let fix = constant
        .filter(|_| !context.tokens().has_comment(range.clone()))
        .map(|constant| Edit::new(range.clone(), format!("{written}.{}", &constant["Boolean.".len()..])));
    diagnostics.push(with_optional_fix(context, diagnostic, fix));
}

/// How PMD shows a `Boolean` argument, and the constant it equals if it is `true`, `false`, `"true"` or `"false"`.
fn literal_value(argument: &Tree, context: &Context) -> (&'static str, Option<&'static str>) {
    match (&argument.kind, context.source(argument)) {
        (K::Literal(LitKind::Boolean), "true") => ("true", Some("Boolean.TRUE")),
        (K::Literal(LitKind::Boolean), _) => ("false", Some("Boolean.FALSE")),
        (K::Literal(LitKind::String), "\"true\"") => ("\"true\"", Some("Boolean.TRUE")),
        (K::Literal(LitKind::String), "\"false\"") => ("\"false\"", Some("Boolean.FALSE")),
        (K::Literal(LitKind::String), _) => ("\"...\"", None),
        _ => ("...", None),
    }
}

/// Whether the argument of `new Float` is not a `double`, so that `Float.valueOf` accepts it.
fn is_float_compatible(argument: &Tree, context: &Context) -> bool {
    let argument = strip_parens(argument);
    let code = context.source(argument);
    match argument.kind {
        K::Literal(LitKind::String | LitKind::Char) => true,
        // `-1` is an operation in tree-sitter's syntax tree, whose behaviour the rule keeps.
        K::Literal(LitKind::Int | LitKind::Long) => !code.starts_with('-'),
        K::Literal(LitKind::Float | LitKind::Double) => code.ends_with(['f', 'F']),
        _ => false,
    }
}

/// The diagnostic with the fix, unless the instance is compared with `==` or `!=`, where the identity of the new
/// instance matters, or is a statement of its own, which a literal or constant cannot be.
fn with_optional_fix(context: &Context, diagnostic: Diagnostic, fix: Option<Edit>) -> Diagnostic {
    let fix = fix.filter(|_| {
        context.parent().is_none_or(|p| match &p.kind {
            // A `case` rule's body is an expression statement in tree-sitter's syntax tree.
            K::ExprStmt(_) | K::Switch { .. } | K::SwitchExpr { .. } => false,
            K::Binary { op, .. } => !matches!(op.as_str(), "==" | "!="),
            _ => true,
        })
    });
    match fix {
        Some(fix) => diagnostic.with_fix(vec![fix]),
        None => diagnostic,
    }
}
