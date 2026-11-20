//! S005: a local variable that is only declared to be returned by the next statement (PMD
//! UnnecessaryLocalBeforeReturn with its defaults): `T x = e; return x;` can be `return e;`. Reported on the variable,
//! like PMD; not if the variable is annotated or used elsewhere.
//!
//! The fix is left out where returning the expression directly could convert it differently: unless the variable's
//! type is written exactly like the method's return type (not in lambdas, not with `var`). It is also left out for
//! array initializers (`int[] a = {1}`), several variables in one declaration, and comments that would be lost.

use std::ops::Range;

use crate::lint::tree::{Context, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

pub fn check(statement: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Return(Some(value)) = &statement.kind else { return };
    let K::Ident(name) = &strip_parens(value).kind else { return };
    let Some((declaration, later)) = previous_statement(statement, context) else { return };
    let K::Vars(declarators) = &declaration.kind else { return };
    let Some(position) = declarators.iter().position(|d| d.name == *name) else { return };
    let declarator = &declarators[position];
    let Some(initializer) = &declarator.init else { return };
    let used_in_later_declarators = declarators
        .get(position + 1)
        .is_some_and(|next| mentions(next.name_pos..declarators[declarators.len() - 1].end, name, context));
    if !declarator.mods.annotations.is_empty()
        || used_in_later_declarators
        || later.is_some_and(|range| mentions(range, name, context))
    {
        return;
    }
    let message = format!("Consider simply returning the value vs storing it in local variable '{name}'");
    let diagnostic = Diagnostic::new(Rule::LocalBeforeReturn, declarator.name_pos, message);
    let tokens = context.tokens();
    let fixable = declarators.len() == 1
        && !matches!(initializer.kind, K::NewArray { elem: None, .. })
        && !tokens.has_comment(declaration.start..declaration.end)
        && !tokens.has_comment(statement.start..statement.end)
        && context.text[declaration.end..statement.start].trim().is_empty()
        && declarator.ty.as_deref().is_some_and(|t| {
            t.end < declarator.name_pos
                && return_type(context).is_some_and(|r| same_type(context.source(r), context.source(t)))
        });
    diagnostics.push(if fixable {
        let fix = Edit::new(declaration.start..statement.end, format!("return {};", context.source(initializer)));
        diagnostic.with_fix(vec![fix])
    } else {
        diagnostic
    });
}

/// The statement before `statement` in its block or `case`, and in a `case`, the range of the following cases, where
/// a variable declared before `statement` is still in scope.
fn previous_statement<'t>(statement: &Tree, context: &Context<'t>) -> Option<(&'t Tree, Option<Range<usize>>)> {
    let parent = context.parent()?;
    let (statements, later) = match &parent.kind {
        K::Block { stmts, .. } => (stmts, None),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            let index = cases.iter().position(|c| c.stmts.iter().any(|s| std::ptr::eq(s, statement)))?;
            (&cases[index].stmts, cases.get(index + 1).map(|next| next.start..parent.end))
        }
        _ => return None,
    };
    let index = statements.iter().position(|s| std::ptr::eq(s, statement))?;
    let previous = statements[..index].iter().rev().find(|s| !matches!(s.kind, K::Empty))?;
    Some((previous, later))
}

/// Whether the name occurs as a token in the range.
fn mentions(range: Range<usize>, name: &str, context: &Context) -> bool {
    context.tokens().in_range(range).any(|t| context.text[t] == *name)
}

/// The declared return type of the method that the statement returns from, unless it is in a lambda.
fn return_type<'t>(context: &Context<'t>) -> Option<&'t Tree> {
    let owner = context.ancestors.iter().rev().find(|a| {
        matches!(a.kind, K::Method(_) | K::Lambda { .. } | K::Class(_))
            || matches!(a.kind, K::NewClass { body: Some(_), .. })
    })?;
    let K::Method(method) = &owner.kind else { return None };
    method.ret.as_deref().filter(|r| r.end < method.name_pos)
}

fn same_type(a: &str, b: &str) -> bool {
    let normalized = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    normalized(a) == normalized(b)
}
