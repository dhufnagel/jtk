//! Checks on the javac-shaped AST of [`crate::ast`], which the hand-written parser builds several times faster than
//! tree-sitter parses. Rules move here from the tree-sitter checks one by one.

use std::ops::Range;

use crate::ast::{K, LitKind, Tree};
use crate::input::{RawKind, lex};
use crate::lint::{Diagnostic, Rule, Selection};

/// What a check sees besides the tree it checks.
pub(super) struct Context<'t> {
    pub text: &'t str,
    /// The trees enclosing the checked one, outermost first.
    pub ancestors: Vec<&'t Tree>,
}

impl<'t> Context<'t> {
    pub fn parent(&self) -> Option<&'t Tree> {
        self.ancestors.last().copied()
    }

    pub fn source(&self, tree: &Tree) -> &'t str {
        &self.text[tree.start..tree.end]
    }
}

pub(super) type TreeCheck = for<'t> fn(&'t Tree, &Context<'t>, &mut Vec<Diagnostic>);

const CHECKS: &[(Rule, TreeCheck)] = &[
    (Rule::EmptyCatch, super::bugs::empty_catch::check),
    (Rule::StringLiteralEquality, super::bugs::string_literal_equality::check),
    (Rule::UnusedLocalVariable, super::bugs::unused_local_variable::check),
    (Rule::SimplifyBooleanReturn, super::simplifications::simplify_boolean_return::check),
];

/// Runs the selected AST checks on the compilation unit.
pub(super) fn check(unit: &Tree, text: &str, selection: &Selection, diagnostics: &mut Vec<Diagnostic>) {
    let checks: Vec<TreeCheck> = CHECKS.iter().filter(|(r, _)| selection.includes(*r)).map(|(_, c)| *c).collect();
    if checks.is_empty() {
        return;
    }
    let mut context = Context { text, ancestors: Vec::new() };
    walk(unit, &mut context, &checks, diagnostics);
}

fn walk<'t>(tree: &'t Tree, context: &mut Context<'t>, checks: &[TreeCheck], diagnostics: &mut Vec<Diagnostic>) {
    for check in checks {
        check(tree, context, diagnostics);
    }
    context.ancestors.push(tree);
    tree.for_each_child(&mut |child| walk(child, context, checks, diagnostics));
    context.ancestors.pop();
}

/// The expression inside any number of parentheses.
pub(super) fn strip_parens(mut tree: &Tree) -> &Tree {
    while let K::Parens(inner) = &tree.kind {
        tree = inner;
    }
    tree
}

/// Whether a comment is in the range of the source.
pub(super) fn has_comment(text: &str, range: Range<usize>) -> bool {
    let part = &text[range];
    (part.contains("//") || part.contains("/*"))
        && lex(part, false).is_ok_and(|(tokens, _)| tokens.iter().any(|t| matches!(t.0, RawKind::Comment)))
}

/// The range of the first token in the range of the source, skipping comments: an operator between two operands.
pub(super) fn first_token(text: &str, range: Range<usize>) -> Option<Range<usize>> {
    let (tokens, _) = lex(&text[range.clone()], false).ok()?;
    let &(_, start, end) = tokens.iter().find(|t| matches!(t.0, RawKind::Token))?;
    Some(range.start + start..range.start + end)
}

pub(super) fn is_boolean_literal(tree: &Tree) -> bool {
    matches!(tree.kind, K::Literal(LitKind::Boolean))
}

/// `!expression`, with parentheses unless the expression is a primary expression.
pub(super) fn negation(expression: &Tree, text: &str) -> String {
    let expression = strip_parens(expression);
    let code = &text[expression.start..expression.end];
    match &expression.kind {
        K::Ident(_) | K::MethodInvocation { .. } | K::ArrayAccess { .. } | K::Literal(LitKind::Boolean) => {
            format!("!{code}")
        }
        K::MemberSelect { name, .. } if name != "class" => format!("!{code}"),
        K::Unary { op, expr, .. } if op == "!" => text[expr.start..expr.end].to_string(),
        _ => format!("!({code})"),
    }
}

/// The trees from `root` (exclusive) down to the smallest tree containing `range`, outermost first.
pub(super) fn path_to(root: &Tree, range: Range<usize>) -> Vec<&Tree> {
    let mut path = Vec::new();
    let mut current = root;
    loop {
        let mut next = None;
        current.for_each_child(&mut |child| {
            if next.is_none() && child.start <= range.start && range.end <= child.end {
                next = Some(child);
            }
        });
        match next {
            Some(child) => {
                path.push(child);
                current = child;
            }
            None => return path,
        }
    }
}

/// Whether `child` of `parent` is a type (or a name that is not an expression, such as an import's), not an
/// expression.
pub(super) fn is_type_child(parent: &Tree, child: &Tree) -> bool {
    let is = |t: &Tree| std::ptr::eq(t, child);
    let var_type = |v: &crate::ast::VarDecl| v.ty.as_deref().is_some_and(is);
    match &parent.kind {
        K::CompilationUnit { package, .. } => package.as_ref().is_some_and(|(_, name)| is(name)),
        K::Import { .. } | K::Module { .. } | K::Directive { .. } | K::Requires { .. } | K::Uses { .. } => true,
        K::Class(c) => {
            c.extends.as_deref().is_some_and(is)
                || c.implements.iter().any(is)
                || c.permits.iter().any(is)
                || c.type_params.iter().any(is)
                || c.record_components.iter().any(var_type)
        }
        K::Method(m) => {
            m.ret.as_deref().is_some_and(is)
                || m.throws.iter().any(is)
                || m.type_params.iter().any(is)
                || m.params.iter().any(var_type)
                || m.receiver.as_deref().is_some_and(var_type)
        }
        K::Vars(vars) => vars.iter().any(var_type),
        K::Var(var) | K::BindingPattern { var } => var_type(var),
        K::ForEach { var, .. } => var_type(var),
        K::Try { catches, .. } => catches.iter().any(|c| var_type(&c.param) || c.union.iter().any(is)),
        K::Lambda { params, .. } => params.iter().any(var_type),
        K::MemberSelect { name, .. } => name == "class",
        K::MethodInvocation { type_args, .. } => type_args.iter().any(is),
        K::NewClass { type_args, ident, .. } => is(ident) || type_args.iter().any(is),
        K::NewArray { elem, .. } => elem.as_deref().is_some_and(is),
        K::MemberRef { qual, type_args, .. } => type_args.iter().any(is) || is(qual) && !qual.is_expression(),
        K::Cast { ty, .. } => is(ty),
        K::InstanceOf { target, .. } => is(target),
        K::RecordPattern { ty, .. } => is(ty),
        K::Annotation { ty, .. } => is(ty),
        K::Primitive(_)
        | K::ArrayType { .. }
        | K::Parameterized { .. }
        | K::Wildcard { .. }
        | K::Annotated { .. }
        | K::Union(_)
        | K::Intersection(_)
        | K::TypeParameter { .. } => !matches!(child.kind, K::Annotation { .. }),
        _ => false,
    }
}

/// Whether the tree at the end of `path` (below `root`) is in a type: a child of a type is a type too, except the
/// annotations on it.
pub(super) fn in_type(root: &Tree, path: &[&Tree]) -> bool {
    let mut in_type = false;
    let mut parent = root;
    for child in path {
        in_type =
            if matches!(child.kind, K::Annotation { .. }) { false } else { in_type || is_type_child(parent, child) };
        parent = child;
    }
    in_type
}
