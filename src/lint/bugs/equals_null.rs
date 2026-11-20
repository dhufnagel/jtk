//! B009: `x.equals(null)` (PMD EqualsNull): is always false, or throws if `x` is null.
//!
//! Unsafe fix: `x == null` (Eclipse's quick fix). It changes the behaviour where `x` is null (no exception) or its
//! `equals` breaks the contract.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::MethodInvocation { select, type_args, args } = &tree.kind else { return };
    let K::MemberSelect { expr: object, name } = &select.kind else { return };
    let tokens = context.tokens();
    // A comment among the arguments counted as one.
    let is_null_argument = matches!(args.as_slice(), [argument] if matches!(argument.kind, K::Literal(LitKind::Null)))
        && !tokens.has_comment(select.end..tree.end);
    if name != "equals" || !is_null_argument {
        return;
    }
    let diagnostic = Diagnostic::new(Rule::EqualsNull, tree.start, "equals(null) instead of == null");
    let parent = context.parent();
    // A comparison is no statement.
    let fixable = !matches!(&object.kind, K::Ident(name) if name == "super")
        && !tokens.has_comment(tree.start..tree.end)
        && type_args.is_empty()
        && parent.is_some_and(|p| !matches!(p.kind, K::ExprStmt(_)));
    diagnostics.push(if fixable {
        // The call binds tighter than `==`: keep the comparison together with parentheses unless the context is
        // known not to need them.
        let comparison = format!("{} == null", context.source(object));
        let needs_parentheses = parent.is_none_or(|p| !is_low_precedence_context(p, tree, context));
        let replacement = if needs_parentheses { format!("({comparison})") } else { comparison };
        diagnostic.with_unsafe_fix(vec![Edit::new(tree.start..tree.end, replacement)])
    } else {
        diagnostic
    });
}

/// Whether an expression can be replaced by a comparison without parentheses in this parent: where it is a whole
/// expression, or an operand of `&&`, `||` or `?:` (conditional operators bind less tightly than `==`).
fn is_low_precedence_context(parent: &Tree, child: &Tree, context: &Context) -> bool {
    let is = |t: &Tree| std::ptr::eq(t, child);
    match &parent.kind {
        K::Parens(_)
        | K::MethodInvocation { .. }
        | K::Vars(_)
        | K::Return(_)
        | K::Lambda { .. }
        | K::Yield(_)
        | K::Assert { .. } => true,
        K::NewClass { args, .. } => args.iter().any(is),
        K::Class(class) => class.enum_constants.iter().flat_map(|c| &c.args).any(is),
        // An annotation's `k = v` is no assignment.
        K::Assign { rhs, .. } => {
            is(rhs) && !context.grandparent().is_some_and(|g| matches!(g.kind, K::Annotation { .. }))
        }
        K::Binary { op, .. } => matches!(op.as_str(), "&&" | "||"),
        K::Conditional { cond, .. } => is(cond),
        _ => false,
    }
}
