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

use super::shared_a::is_varargs;
use crate::ast::{ClassKind, K, LitKind, Tree, VarDecl};
use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { select, type_args, args } = &tree.kind else { return };
    let K::MemberSelect { expr: object, name: method } = &select.kind else { return };
    if !matches!(method.as_str(), "equals" | "equalsIgnoreCase") {
        return;
    }
    let tokens = context.tokens();
    let [argument] = args.as_slice() else { return };
    // The cheap test first: resolving the receiver walks the enclosing scopes.
    if is_literal_argument(argument) && is_string_variable(object, context) {
        let message = format!("String literal expressions should be on the left side of an {method} comparison");
        let parenthesis = tokens.next(select.end).map_or(select.end, |p| p.start);
        let diagnostic = Diagnostic::new(Rule::EqualsAvoidNull, parenthesis, message);
        diagnostics.push(if tokens.has_comment(tree.start..tree.end) || !type_args.is_empty() {
            diagnostic
        } else {
            let receiver = match argument.kind {
                K::Literal(LitKind::String) | K::Parens(_) => context.source(argument).to_string(),
                _ => format!("({})", context.source(argument)),
            };
            let replacement = format!("{receiver}.{method}({})", context.source(object));
            diagnostic.with_unsafe_fix(vec![Edit::new(tree.start..tree.end, replacement)])
        });
    }
}

/// Checkstyle's test for an argument that cannot be null.
fn is_literal_argument(argument: &Tree) -> bool {
    let mut argument = strip_parens(argument);
    if let K::Assign { op, rhs, .. } = &argument.kind
        && op == "="
    {
        // Checkstyle does not look into parentheses around the assigned value.
        argument = rhs;
    }
    match &argument.kind {
        K::Literal(LitKind::String) => true,
        K::Binary { op, lhs, rhs } if op == "+" => [lhs, rhs].iter().any(|operand| match &strip_parens(operand).kind {
            K::Literal(LitKind::String) => true,
            K::Ident(name) => name != "this" && name != "super",
            _ => false,
        }),
        _ => false,
    }
}

/// A scope of Checkstyle's: a tree or one of the parts of trees that are no trees of their own.
#[derive(Clone, Copy)]
enum Scope<'t> {
    Tree(&'t Tree),
    Catch(&'t VarDecl),
    EnumConstant(&'t crate::ast::EnumConstant),
}

/// Whether the receiver is a name, `this.name` or `Class.name` declared with the type `String`.
fn is_string_variable(object: &Tree, context: &Context) -> bool {
    let (name, qualifier) = match &object.kind {
        K::Ident(name) if name != "this" && name != "super" => (name, None),
        K::MemberSelect { expr, name } => match &expr.kind {
            K::Ident(qualifier) if qualifier != "super" => (name, Some(qualifier.as_str())),
            _ => return false,
        },
        _ => return false,
    };
    for scope in scopes(&context.ancestors) {
        let class_name = class_name(scope);
        let declared = || declaration(scope, name, context.text);
        match qualifier {
            None => {
                if let Some((offset, is_string)) = declared()
                    && (class_name.is_some() || offset < object.start)
                {
                    return is_string;
                }
            }
            // `this.name` refers to the innermost class, `Class.name` to the enclosing class of that name.
            Some(qualifier) => {
                if class_name.is_some_and(|c| qualifier == "this" || c == qualifier) {
                    return declared().is_some_and(|(_, is_string)| is_string);
                }
            }
        }
    }
    false
}

/// The scopes enclosing the checked tree, innermost first.
fn scopes<'t>(ancestors: &[&'t Tree]) -> impl Iterator<Item = Scope<'t>> {
    (0..ancestors.len()).rev().flat_map(move |index| {
        let tree = ancestors[index];
        let child = ancestors.get(index + 1).copied();
        let is = move |t: &Tree| child.is_some_and(|c| std::ptr::eq(c, t));
        // The parts between the tree and the child come first.
        let part = match &tree.kind {
            K::Try { catches, .. } => catches.iter().find(|c| is(&c.block)).map(|c| Scope::Catch(&c.param)),
            K::Class(class) => class
                .enum_constants
                .iter()
                .find(|c| c.args.iter().chain(c.body.iter().flatten()).any(is))
                .map(Scope::EnumConstant),
            _ => None,
        };
        let own = is_scope(tree).then_some(Scope::Tree(tree));
        part.into_iter().chain(own)
    })
}

/// The name of a class, enum or record, whose fields are visible anywhere in it.
fn class_name<'t>(scope: Scope<'t>) -> Option<&'t str> {
    match scope {
        Scope::Tree(Tree { kind: K::Class(class), .. })
            if matches!(class.kind, ClassKind::Class | ClassKind::Enum | ClassKind::Record) =>
        {
            Some(&class.name)
        }
        _ => None,
    }
}

fn is_scope(tree: &Tree) -> bool {
    matches!(
        tree.kind,
        K::Class(_)
            | K::Method(_)
            | K::Lambda { .. }
            | K::For { .. }
            | K::ForEach { .. }
            | K::Try { .. }
            | K::Switch { .. }
            | K::SwitchExpr { .. }
            | K::Block { .. }
            | K::NewClass { body: Some(_), .. }
    )
}

/// The last declaration of the name in the scope, outside nested scopes: the offset of its name and whether its type
/// is `String`. A later declaration of a name replaces an earlier one, as in Checkstyle.
fn declaration(scope: Scope, name: &str, text: &str) -> Option<(usize, bool)> {
    let mut found = None;
    let mut declare = |var: &VarDecl, is_string: bool| {
        if var.name == name {
            found = Some((var.name_pos, is_string));
        }
    };
    match scope {
        // The type of a catch parameter is never a plain `String` to Checkstyle.
        Scope::Catch(parameter) => declare(parameter, false),
        Scope::EnumConstant(constant) => {
            for tree in constant.args.iter().chain(constant.body.iter().flatten()) {
                declarations(tree, text, &mut declare);
            }
        }
        Scope::Tree(tree) => match &tree.kind {
            K::Class(class) => {
                class.record_components.iter().for_each(|v| declare(v, is_string_type(v)));
                class.members.iter().for_each(|m| declarations(m, text, &mut declare));
            }
            K::Try { resources, .. } => resources.iter().for_each(|r| declarations(r, text, &mut declare)),
            _ => {
                declare_own(tree, text, &mut declare);
                tree.for_each_child(&mut |child| declarations(child, text, &mut declare));
            }
        },
    }
    found
}

/// Declares the variables of the tree and its descendants outside nested scopes.
fn declarations(tree: &Tree, text: &str, declare: &mut impl FnMut(&VarDecl, bool)) {
    if is_scope(tree) {
        return;
    }
    declare_own(tree, text, declare);
    tree.for_each_child(&mut |child| declarations(child, text, declare));
}

/// Declares the variables the tree itself declares.
fn declare_own(tree: &Tree, text: &str, declare: &mut impl FnMut(&VarDecl, bool)) {
    match &tree.kind {
        K::Vars(vars) => vars.iter().for_each(|v| declare(v, is_string_type(v))),
        K::Var(var) | K::ForEach { var, .. } | K::BindingPattern { var } => declare(var, is_string_type(var)),
        K::Method(method) => method.params.iter().for_each(|p| declare(p, !is_varargs(p, text) && is_string_type(p))),
        // Parameters without types hide outer variables (a single one without parentheses does not in Checkstyle).
        K::Lambda { params, .. } if text[tree.start..].starts_with('(') => {
            params.iter().for_each(|p| declare(p, !is_varargs(p, text) && is_string_type(p)))
        }
        _ => {}
    }
}

/// Whether the declared type is `String` (Checkstyle compares the simple type name, ignoring array dimensions).
fn is_string_type(var: &VarDecl) -> bool {
    let Some(mut ty) = var.ty.as_deref() else { return false };
    while let K::ArrayType { elem, .. } = &ty.kind {
        ty = elem;
    }
    matches!(&ty.kind, K::Ident(name) if name == "String")
}
