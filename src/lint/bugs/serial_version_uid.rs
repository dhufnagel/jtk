//! B241: a field `serialVersionUID` that is not `private static final long`: serialization ignores it unless it is
//! `static final long`, and it should be `private` as subclasses need their own (IntelliJ
//! SerialVersionUIDNotStaticFinal, javac `-Xlint:serial` improper.SVUID, long.SVUID and constant.SVUID).
//!
//! Only fields of classes are checked, not of interfaces (implicitly `public static final`), records and enums (whose
//! `serialVersionUID` serialization ignores) or anonymous classes; whether the class is `Serializable` is not known.
//!
//! The fix declares the field `private static final`, keeping annotations and the type, for a single `long` field
//! without comments, `transient` or `volatile`. Adding only `private` is safe (serialization does not care about the
//! access, and the field belongs to the class's serialization alone; another file using it would no longer compile).
//! Adding `static` or `final` is unsafe: the declared value then replaces the computed one, which changes the
//! serialized form's compatibility with existing data. It is offered only for a literal initializer that nothing in
//! the file assigns, so that the field compiles as a constant. A field of another type gets no fix.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{ClassKind, K, LitKind, Modifier, Tree, VarDecl};

const NAME: &str = "serialVersionUID";

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Vars(vars) = &tree.kind else { return };
    let Some(var) = vars.iter().find(|v| v.name == NAME) else { return };
    let in_class = context.parent().is_some_and(|p| matches!(&p.kind, K::Class(c) if c.kind == ClassKind::Class));
    let Some(ty) = var.ty.as_deref() else { return };
    let is_long = context.source(ty) == "long";
    let mods = &var.mods;
    let is_constant = mods.has(Modifier::Static) && mods.has(Modifier::Final);
    if !in_class || is_long && is_constant && mods.has(Modifier::Private) {
        return;
    }
    let diagnostic =
        Diagnostic::new(Rule::SerialVersionUid, tree.start, "serialVersionUID is not private static final long");
    let fixable = is_long
        && vars.len() == 1
        && !mods.has(Modifier::Transient)
        && !mods.has(Modifier::Volatile)
        && !context.tokens().has_comment(tree.start..tree.end);
    diagnostics.push(match fixable {
        true if is_constant => diagnostic.with_fix(modifier_edits(var, context)),
        true if has_literal_initializer(var) && !is_assigned(context.ancestors[0]) => {
            diagnostic.with_unsafe_fix(modifier_edits(var, context))
        }
        _ => diagnostic,
    });
}

/// The edits that replace the access and `static` and `final` modifiers by `private static final`.
fn modifier_edits(var: &VarDecl, context: &Context) -> Vec<Edit> {
    let replaced = [Modifier::Public, Modifier::Protected, Modifier::Private, Modifier::Static, Modifier::Final];
    let tokens = context.tokens();
    let mut edits: Vec<Edit> = var
        .mods
        .keywords
        .iter()
        .filter(|(modifier, _)| replaced.contains(modifier))
        .map(|(_, range)| {
            let end = tokens.next(range.end).map_or(range.end, |next| next.start);
            Edit::new(range.start..end, "")
        })
        .collect();
    let first_keyword = var.mods.keywords.first().map(|(_, range)| range.start);
    let insertion = first_keyword.or(var.ty.as_ref().map(|ty| ty.start)).unwrap_or(var.start);
    match edits.first_mut() {
        Some(first) if first.range.start == insertion => first.replacement = "private static final ".to_string(),
        _ => edits.insert(0, Edit::new(insertion..insertion, "private static final ")),
    }
    edits
}

/// Whether the variable is initialized with a (possibly negated) integer literal.
fn has_literal_initializer(var: &VarDecl) -> bool {
    let Some(init) = var.init.as_deref() else { return false };
    let init = match &init.kind {
        K::Unary { op, expr, .. } if op == "-" => expr,
        _ => init,
    };
    matches!(init.kind, K::Literal(LitKind::Int | LitKind::Long))
}

/// Whether something in the tree assigns a variable named `serialVersionUID`.
fn is_assigned(tree: &Tree) -> bool {
    let target = match &tree.kind {
        K::Assign { lhs, .. } => Some(lhs),
        K::Unary { op, expr, .. } if op == "++" || op == "--" => Some(expr),
        _ => None,
    };
    let assigns = target.is_some_and(|t| match &t.kind {
        K::Ident(name) | K::MemberSelect { name, .. } => name == NAME,
        _ => false,
    });
    let mut assigned = assigns;
    if !assigned {
        tree.for_each_child(&mut |child| assigned = assigned || is_assigned(child));
    }
    assigned
}
