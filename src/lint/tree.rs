//! The checks of all rules run on the javac-shaped AST of [`crate::ast`] that the hand-written parser builds, with
//! the file's tokens and comments for what the tree leaves out. This module walks the tree and holds what the
//! checks share.

use std::cell::OnceCell;
use std::ops::Range;
use std::path::Path;

use crate::ast::{K, LitKind, Mods, Tag, Tree};
use crate::input::{RawKind, lex};
use crate::lint::{Diagnostic, Rule, RuleOptions, Selection};

/// What a check sees besides the tree it checks.
pub(super) struct Context<'t> {
    pub text: &'t str,
    /// The trees enclosing the checked one, outermost first: the compilation unit is the first.
    pub ancestors: Vec<&'t Tree>,
    pub options: &'t RuleOptions,
    /// The path of the checked file, if known.
    pub path: Option<&'t Path>,
    tokens: OnceCell<Tokens>,
}

impl<'t> Context<'t> {
    pub fn new(text: &'t str, options: &'t RuleOptions, path: Option<&'t Path>) -> Context<'t> {
        Context { text, ancestors: Vec::new(), options, path, tokens: OnceCell::new() }
    }

    pub fn parent(&self) -> Option<&'t Tree> {
        self.ancestors.last().copied()
    }

    /// The parent of the parent.
    pub fn grandparent(&self) -> Option<&'t Tree> {
        self.ancestors.len().checked_sub(2).map(|i| self.ancestors[i])
    }

    pub fn source(&self, tree: &Tree) -> &'t str {
        &self.text[tree.start..tree.end]
    }

    /// The tokens and comments of the file, lexed on first use.
    pub fn tokens(&self) -> &Tokens {
        self.tokens.get_or_init(|| Tokens::new(self.text))
    }
}

/// The tokens and comments of a file, in order, without whitespace.
pub(super) struct Tokens {
    /// Each token's range, and whether it is a comment.
    all: Vec<(Range<usize>, bool)>,
}

impl Tokens {
    pub fn new(text: &str) -> Tokens {
        let raw = lex(text, false).map(|(raw, _)| raw).unwrap_or_default();
        let all = raw
            .into_iter()
            .filter_map(|(kind, start, end)| match kind {
                RawKind::Whitespace => None,
                RawKind::Comment => Some((start..end, true)),
                RawKind::Token => Some((start..end, false)),
            })
            .collect();
        Tokens { all }
    }

    /// The index of the first token or comment starting at or after `offset`.
    fn index(&self, offset: usize) -> usize {
        self.all.partition_point(|(r, _)| r.start < offset)
    }

    /// The comments starting in the range.
    pub fn comments_in(&self, range: Range<usize>) -> impl Iterator<Item = Range<usize>> + '_ {
        let end = range.end;
        self.all[self.index(range.start)..]
            .iter()
            .take_while(move |(r, _)| r.start < end)
            .filter(|(_, c)| *c)
            .map(|(r, _)| r.clone())
    }

    /// Whether a comment starts in the range.
    pub fn has_comment(&self, range: Range<usize>) -> bool {
        self.comments_in(range).next().is_some()
    }

    /// The tokens (not comments) starting in the range.
    pub fn in_range(&self, range: Range<usize>) -> impl Iterator<Item = Range<usize>> + '_ {
        let end = range.end;
        self.all[self.index(range.start)..]
            .iter()
            .take_while(move |(r, _)| r.start < end)
            .filter(|(_, c)| !*c)
            .map(|(r, _)| r.clone())
    }

    /// The first token (not comment) starting at or after `offset`.
    pub fn next(&self, offset: usize) -> Option<Range<usize>> {
        self.all[self.index(offset)..].iter().find(|(_, c)| !*c).map(|(r, _)| r.clone())
    }

    /// The last token (not comment) ending at or before `offset`.
    pub fn previous(&self, offset: usize) -> Option<Range<usize>> {
        let i = self.all.partition_point(|(r, _)| r.end <= offset);
        self.all[..i].iter().rev().find(|(_, c)| !*c).map(|(r, _)| r.clone())
    }

    /// All comments.
    pub fn comments(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.all.iter().filter(|(_, c)| *c).map(|(r, _)| r.clone())
    }
}

/// A check of the trees of the kinds it is registered for.
pub(super) type TreeCheck = for<'t> fn(&'t Tree, &Context<'t>, &mut Vec<Diagnostic>);

/// The groups' checks: each rule with the kinds of trees it checks.
const CHECKS: &[&[(Rule, &[Tag], TreeCheck)]] = &[
    super::imports::TREE_CHECKS,
    super::bugs::TREE_CHECKS,
    super::simplifications::TREE_CHECKS,
    super::modernization::TREE_CHECKS,
    super::naming::TREE_CHECKS,
    super::design::TREE_CHECKS,
    super::complexity::TREE_CHECKS,
];

/// Runs the selected checks on the compilation unit.
pub(super) fn check<'t>(
    unit: &'t Tree,
    context: &mut Context<'t>,
    selection: &Selection,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut by_tag: Vec<Vec<TreeCheck>> = vec![Vec::new(); Tag::COUNT];
    for (rule, tags, check) in CHECKS.iter().copied().flatten() {
        if selection.includes(*rule) {
            for tag in *tags {
                by_tag[*tag as usize].push(*check);
            }
        }
    }
    if by_tag.iter().all(Vec::is_empty) {
        return;
    }
    walk(unit, context, &by_tag, diagnostics);
}

/// Runs the checks on `root` and the trees below it, in document order. Iterative, so that deeply nested code (long
/// chains of `+`) cannot overflow the stack.
fn walk<'t>(root: &'t Tree, context: &mut Context<'t>, checks: &[Vec<TreeCheck>], diagnostics: &mut Vec<Diagnostic>) {
    let children = |tree: &'t Tree| {
        let mut children = Vec::new();
        tree.for_each_child(&mut |child| children.push(child));
        children.into_iter()
    };
    for check in &checks[root.tag() as usize] {
        check(root, context, diagnostics);
    }
    context.ancestors.push(root);
    let mut stack = vec![children(root)];
    while let Some(siblings) = stack.last_mut() {
        match siblings.next() {
            Some(tree) => {
                for check in &checks[tree.tag() as usize] {
                    check(tree, context, diagnostics);
                }
                context.ancestors.push(tree);
                stack.push(children(tree));
            }
            None => {
                stack.pop();
                context.ancestors.pop();
            }
        }
    }
}

/// The modifier keywords of a declaration (`public`, `static`, `non-sealed`, ...) with their ranges, in order:
/// the tokens from the start of the modifiers that are not in an annotation.
pub(super) fn modifier_keywords<'a>(mods: &Mods, text: &'a str, tokens: &Tokens) -> Vec<(&'a str, Range<usize>)> {
    const KEYWORDS: &[&str] = &[
        "public",
        "protected",
        "private",
        "static",
        "abstract",
        "final",
        "native",
        "synchronized",
        "transient",
        "volatile",
        "strictfp",
        "default",
        "sealed",
    ];
    let mut keywords = Vec::new();
    let mut offset = mods.start;
    while let Some(token) = tokens.next(offset) {
        offset = token.end;
        if let Some(annotation) = mods.annotations.iter().find(|a| a.start <= token.start && token.start < a.end) {
            offset = annotation.end;
            continue;
        }
        let word = &text[token.clone()];
        if KEYWORDS.contains(&word) {
            keywords.push((word, token));
        } else if word == "non" && text[token.end..].starts_with("-sealed") {
            let range = token.start..token.end + "-sealed".len();
            offset = range.end;
            keywords.push((&text[range.clone()], range));
        } else {
            break;
        }
    }
    keywords
}

/// Whether the declaration has the modifier keyword.
pub(super) fn has_modifier(mods: &Mods, text: &str, tokens: &Tokens, keyword: &str) -> bool {
    modifier_keywords(mods, text, tokens).iter().any(|(k, _)| *k == keyword)
}

/// Calls `f` on each descendant of `tree` (not `tree` itself) in source order, with its parent; `f` returns
/// whether to visit the descendant's children too.
pub(super) fn for_each_descendant<'t>(tree: &'t Tree, f: &mut impl FnMut(&'t Tree, &'t Tree) -> bool) {
    tree.for_each_child(&mut |child| {
        if f(child, tree) {
            for_each_descendant(child, f);
        }
    });
}

/// The expression inside any number of parentheses.
pub(super) fn strip_parens(mut tree: &Tree) -> &Tree {
    while let K::Parens(inner) = &tree.kind {
        tree = inner;
    }
    tree
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

/// The range that removes a span of source, such as a declaration with its leading comments: its whole lines if
/// nothing else is on them, otherwise the span with the spaces after it.
pub(super) fn span_removal_range(span: Range<usize>, text: &str) -> Range<usize> {
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
