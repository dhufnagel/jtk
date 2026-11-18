//! M001: `instanceof` followed by a cast to a new variable (Eclipse "Use pattern matching for instanceof", IntelliJ
//! "Pattern variable can be used"), Java 16:
//!
//! `if (o instanceof String) { String s = (String) o; ... }` becomes `if (o instanceof String s) { ... }`.
//!
//! The `instanceof` is the condition or part of an `&&` condition, the declaration is the first statement of the
//! `if` block, and the tested expression is a name or field, so that casting it again gives the same object. The
//! rest of the condition must not use the variable's name, which would then refer to the pattern variable.

use crate::ast::{K, Tree};
use crate::lint::tree::{Context, modifier_keywords, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(statement: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::If { cond, then, .. } = &statement.kind else { return };
    let K::Block { stmts, .. } = &then.kind else { return };
    let Some(declaration) = stmts.iter().find(|s| !matches!(s.kind, K::Empty)) else { return };
    let K::Vars(vars) = &declaration.kind else { return };
    let [var] = vars.as_slice() else { return };
    if !var.mods.annotations.is_empty() || !modifier_keywords(&var.mods, context.text, context.tokens()).is_empty() {
        return;
    }
    let Some((cast_type, operand)) = cast(var, context) else { return };
    let name = var.name.as_str();
    let mut tests = Vec::new();
    conjuncts(cond, &mut tests);
    for test in tests {
        let K::InstanceOf { expr: tested, target } = &test.kind else { continue };
        let tested = strip_parens(tested);
        let matches = !matches!(target.kind, K::BindingPattern { .. } | K::RecordPattern { .. })
            && is_name_or_field(tested)
            && context.source(target) == cast_type
            && context.source(tested) == operand
            && !names_after(cond, test.end, name, context);
        if matches {
            let message = format!("instanceof and cast can be a pattern variable: instanceof {cast_type} {name}");
            let fix = vec![
                Edit::new(target.end..target.end, format!(" {name}")),
                Edit::new(statement_lines(declaration, context.text), ""),
            ];
            diagnostics.push(Diagnostic::new(Rule::InstanceofPattern, test.start, message).with_fix(fix));
            return;
        }
    }
}

/// A name or field access (`a`, `this.a`, `a.b`), not `this` or a class literal.
fn is_name_or_field(tree: &Tree) -> bool {
    match &tree.kind {
        K::Ident(name) => name != "this" && name != "super",
        K::MemberSelect { name, .. } => name != "class",
        _ => false,
    }
}

/// Whether the condition names `name` after `position`: the pattern variable would take its place.
fn names_after(condition: &Tree, position: usize, name: &str, context: &Context) -> bool {
    context.tokens().in_range(position..condition.end).any(|token| &context.text[token] == name)
}

/// `T name = (T) x;`: the cast's type and operand.
fn cast<'a>(var: &crate::ast::VarDecl, context: &Context<'a>) -> Option<(&'a str, &'a str)> {
    let K::Cast { ty, expr } = &strip_parens(var.init.as_deref()?).kind else { return None };
    let cast_type = context.source(ty);
    let declared_type = var.ty.as_deref().map_or("var", |t| context.source(t));
    (declared_type == cast_type || declared_type == "var").then(|| (cast_type, context.source(strip_parens(expr))))
}

/// The operands of a condition's top-level `&&` chain (the condition itself if it is not one).
fn conjuncts<'t>(condition: &'t Tree, operands: &mut Vec<&'t Tree>) {
    let condition = strip_parens(condition);
    match &condition.kind {
        K::Binary { op, lhs, rhs } if op == "&&" => {
            conjuncts(lhs, operands);
            conjuncts(rhs, operands);
        }
        _ => operands.push(condition),
    }
}

/// The statement's range, extended to its whole lines when nothing else is on them.
fn statement_lines(statement: &Tree, text: &str) -> std::ops::Range<usize> {
    let line_start = text[..statement.start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = text[statement.end..].find('\n').map_or(text.len(), |i| statement.end + i + 1);
    let alone_on_lines =
        text[line_start..statement.start].trim().is_empty() && text[statement.end..line_end].trim().is_empty();
    if alone_on_lines { line_start..line_end } else { statement.start..statement.end }
}
