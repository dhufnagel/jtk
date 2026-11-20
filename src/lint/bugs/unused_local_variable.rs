//! B026: local variable that is declared, and maybe assigned, but never read (Checkstyle UnusedLocalVariable, PMD
//! UnusedLocalVariable, Error Prone UnusedVariable). Covers `for` variables and pattern variables; `_` is unnamed.
//!
//! Unsafe fix (Eclipse save action "Remove unused local variables"): removes the declaration of a variable that is
//! never assigned afterwards, if its value has no side effects ([`is_side_effect_free`]); from a declaration of several
//! variables only its declarator. Unsafe because a name in the value could be a field whose access loads a class.
//! Eclipse also removes assignments; jtk leaves such variables.

use std::ops::Range;

use crate::lint::tree::{Context, in_type, path_to, span_removal_range, strip_parens};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{CaseKind, ClassKind, K, LitKind, Tree, VarDecl};

/// Where a variable is visible: a range of the source below a tree.
struct Scope<'t> {
    root: &'t Tree,
    range: Range<usize>,
}

impl<'t> Scope<'t> {
    fn of(tree: &'t Tree) -> Scope<'t> {
        Scope { root: tree, range: tree.start..tree.end }
    }
}

pub fn check<'t>(tree: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let text = context.text;
    let declared: Vec<(&VarDecl, Scope)> = match &tree.kind {
        K::Vars(vars) => {
            let scope = match context.parent() {
                Some(parent @ Tree { kind: K::Block { .. } | K::For { .. }, .. }) => Scope::of(parent),
                Some(parent @ Tree { kind: K::Switch { .. } | K::SwitchExpr { .. }, .. }) => Scope::of(parent),
                _ => return,
            };
            vars.iter().map(|v| (v, Scope { root: scope.root, range: scope.range.clone() })).collect()
        }
        K::ForEach { var, .. } => vec![(&**var, Scope::of(tree))],
        K::BindingPattern { var } => vec![(&**var, pattern_scope(tree, &context.ancestors))],
        _ => return,
    };
    for (var, scope) in declared {
        if var.name == "_" {
            continue;
        }
        let references = references(&var.name, &scope, var.name_pos + var.name.len(), text);
        if references.read {
            continue;
        }
        let message = format!("Unused local variable '{}'", var.name);
        let diagnostic = Diagnostic::new(Rule::UnusedLocalVariable, var.name_pos, message);
        let fix = if references.written { None } else { removal(tree, var, context) };
        diagnostics.push(match fix {
            Some(edit) => diagnostic.with_unsafe_fix(vec![edit]),
            None => diagnostic,
        });
    }
}

/// The edit that removes the declaration of `var` from a local variable declaration statement, if its value has no
/// side effects.
fn removal(declaration: &Tree, var: &VarDecl, context: &Context) -> Option<Edit> {
    let K::Vars(declarators) = &declaration.kind else { return None };
    let text = context.text;
    if context.tokens().has_comment(declaration.start..declaration.end) {
        return None;
    }
    let in_statement_list =
        context.parent().is_some_and(|p| matches!(p.kind, K::Block { .. } | K::Switch { .. } | K::SwitchExpr { .. }));
    let value_is_pure = var.init.as_deref().is_none_or(|v| is_side_effect_free(v, text));
    if !in_statement_list || !value_is_pure {
        return None;
    }
    let index = declarators.iter().position(|d| std::ptr::eq(d, var))?;
    let range = match (index.checked_sub(1).map(|i| &declarators[i]), declarators.get(index + 1)) {
        (_, Some(next)) => var.name_pos..next.name_pos,
        (Some(previous), None) => previous.end..var.end,
        (None, None) => span_removal_range(declaration.start..declaration.end, text),
    };
    Some(Edit::new(range, ""))
}

/// Whether evaluating the expression has no effect: a name, a literal or a field of one.
fn is_side_effect_free(expression: &Tree, text: &str) -> bool {
    let literal = || &text[expression.start..expression.end];
    match &expression.kind {
        K::Ident(name) => name != "super",
        K::Literal(LitKind::Null | LitKind::Boolean | LitKind::String | LitKind::Char) => true,
        // Like tree-sitter's decimal and hex integer literals and decimal floating point literals.
        K::Literal(LitKind::Int | LitKind::Long) => {
            let literal = literal();
            !literal.starts_with('0') || literal.len() == 1 || literal[1..].starts_with(['l', 'L', 'x', 'X'])
        }
        K::Literal(LitKind::Float | LitKind::Double) => !literal().starts_with("0x") && !literal().starts_with("0X"),
        K::MemberSelect { expr, name } => name != "class" && is_side_effect_free(expr, text),
        K::Parens(_) => is_side_effect_free(strip_parens(expression), text),
        _ => false,
    }
}

/// Where a pattern variable is visible: its `case`, or the statement with the `instanceof`. After an
/// `if (!(o instanceof T t))`, `t` stays visible in the rest of the block.
fn pattern_scope<'t>(pattern: &'t Tree, ancestors: &[&'t Tree]) -> Scope<'t> {
    let chain: Vec<&Tree> = ancestors.iter().copied().chain([pattern]).collect();
    for index in (0..chain.len() - 1).rev() {
        let (node, child) = (chain[index], chain[index + 1]);
        // A case is between a switch and the trees in it.
        if let Some(range) = case_range(node, child) {
            return Scope { root: node, range };
        }
        let parent = index.checked_sub(1).map(|i| chain[i]);
        let in_list = parent.is_some_and(|p| is_in_statement_list(p, node));
        if !matches!(node.kind, K::Lambda { .. }) && !in_list {
            continue;
        }
        let in_negated_condition = matches!(&node.kind, K::If { cond, .. }
            if matches!(&strip_parens(cond).kind, K::Unary { op, .. } if op == "!"));
        let Some(parent) = parent.filter(|_| in_negated_condition) else { return Scope::of(node) };
        return match case_range(parent, node) {
            Some(range) => Scope { root: parent, range },
            None => Scope::of(parent),
        };
    }
    Scope::of(pattern)
}

/// Whether the statement is in a block or in the statements of a `case ... :`.
fn is_in_statement_list(parent: &Tree, statement: &Tree) -> bool {
    match &parent.kind {
        K::Block { .. } => true,
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            cases.iter().any(|c| c.kind == CaseKind::Statement && c.stmts.iter().any(|s| std::ptr::eq(s, statement)))
        }
        _ => false,
    }
}

/// The range of the case of the switch that `child` is in, if `node` is a switch.
fn case_range(node: &Tree, child: &Tree) -> Option<Range<usize>> {
    let (K::Switch { cases, .. } | K::SwitchExpr { cases, .. }) = &node.kind else { return None };
    let is = |t: &Tree| std::ptr::eq(t, child);
    let case = cases.iter().find(|c| {
        c.labels.iter().any(is)
            || c.guard.as_deref().is_some_and(is)
            || c.stmts.iter().any(is)
            || c.body.as_deref().is_some_and(is)
    })?;
    let ends = case.labels.iter().chain(case.guard.as_deref()).chain(&case.stmts).chain(case.body.as_deref());
    Some(case.start..ends.map(|t| t.end).max().unwrap_or(case.start))
}

#[derive(Default)]
struct References {
    read: bool,
    written: bool,
}

/// Whether the variable is read, and whether it is written, after `after` in its scope. Code in a nested class that
/// declares the same name refers to its own variable.
///
/// Only identifiers spelled like the variable can refer to it, so its occurrences are found in the text instead of
/// walking the scope: an occurrence counts if the scope's smallest tree around it is exactly an identifier.
fn references(variable: &str, scope: &Scope, after: usize, text: &str) -> References {
    let mut references = References::default();
    let start = after.max(scope.range.start);
    let end = scope.range.end;
    if start >= end || variable.is_empty() {
        return references;
    }
    // Classes already checked for declaring the variable.
    let mut classes: Vec<(*const Tree, bool)> = Vec::new();
    let mut from = start;
    while let Some(i) = text[from..end].find(variable) {
        let position = from + i;
        from = position + variable.len();
        let path = path_to(scope.root, position..from);
        let Some((&identifier, ancestors)) = path.split_last() else { continue };
        if !matches!(identifier.kind, K::Ident(_)) || identifier.start != position || identifier.end != from {
            continue;
        }
        if in_shadowing_class(&path, variable, &mut classes) || !is_name_reference(scope.root, &path) {
            continue;
        }
        let parent = ancestors.last().copied().unwrap_or(scope.root);
        let grandparent = ancestors.len().checked_sub(2).map_or(scope.root, |i| ancestors[i]);
        if is_only_written(identifier, parent, grandparent) {
            references.written = true;
        } else {
            references.read = true;
            return references;
        }
    }
    references
}

/// Whether the identifier at the end of the path is the name of a variable, not of a type, method or label.
fn is_name_reference(root: &Tree, path: &[&Tree]) -> bool {
    let identifier = path[path.len() - 1];
    let parent = path.len().checked_sub(2).map_or(root, |i| path[i]);
    match &parent.kind {
        K::MethodInvocation { select, .. } if std::ptr::eq(&**select, identifier) => return false,
        K::Assign { lhs, .. } if std::ptr::eq(&**lhs, identifier) => {
            let grandparent = path.len().checked_sub(3).map_or(root, |i| path[i]);
            if matches!(grandparent.kind, K::Annotation { .. }) {
                return false;
            }
        }
        _ => {}
    }
    !in_type(root, path)
}

/// Whether the identifier is only written: the target of a plain assignment, or incremented (`i++`) without using
/// the result. Like Checkstyle, a compound assignment (`x += 1`) counts as a use.
fn is_only_written(identifier: &Tree, parent: &Tree, grandparent: &Tree) -> bool {
    match &parent.kind {
        K::Assign { op, lhs, .. } => std::ptr::eq(&**lhs, identifier) && op == "=",
        // As a statement: also the body of a `case ->`, which tree-sitter wraps in an expression statement.
        K::Unary { op, .. } if op == "++" || op == "--" => match &grandparent.kind {
            K::ExprStmt(_) | K::For { .. } => true,
            K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
                cases.iter().any(|c| c.body.as_deref().is_some_and(|b| std::ptr::eq(b, parent)))
            }
            _ => false,
        },
        _ => false,
    }
}

/// Whether the identifier at the end of the path is in the body of a class (below the path's root) that declares a
/// variable of the same name.
fn in_shadowing_class(path: &[&Tree], variable: &str, checked: &mut Vec<(*const Tree, bool)>) -> bool {
    for (index, &tree) in path.iter().enumerate().rev().skip(1) {
        let child = path[index + 1];
        let Some(body) = class_body(tree, child) else { continue };
        let declares = match checked.iter().find(|(t, _)| std::ptr::eq(*t, tree)) {
            Some(&(_, declares)) => declares,
            None => {
                let declares = body.iter().any(|m| declares(m, variable));
                checked.push((tree, declares));
                declares
            }
        };
        if declares {
            return true;
        }
    }
    false
}

/// The members of the class body of `tree` that `child` is in, if any: the body of a class or record, of an
/// anonymous class or of an enum constant (not of an interface or enum, as tree-sitter has other kinds of bodies for
/// them).
fn class_body<'t>(tree: &'t Tree, child: &Tree) -> Option<&'t [Tree]> {
    let is = |t: &Tree| std::ptr::eq(t, child);
    match &tree.kind {
        K::Class(class) if matches!(class.kind, ClassKind::Class | ClassKind::Record) => {
            class.members.iter().any(is).then_some(&class.members[..])
        }
        K::Class(class) if class.kind == ClassKind::Enum => {
            class.enum_constants.iter().filter_map(|c| c.body.as_deref()).find(|body| body.iter().any(is))
        }
        K::NewClass { body: Some(body), .. } => body.iter().any(is).then_some(&body[..]),
        _ => None,
    }
}

/// Whether the tree declares a field, local variable or parameter named `variable` (as tree-sitter's variable
/// declarators and formal parameters: not resources, catch, `for` or pattern variables or untyped lambda parameters).
fn declares(tree: &Tree, variable: &str) -> bool {
    let named = |v: &VarDecl| v.name == variable;
    let here = match &tree.kind {
        K::Vars(vars) => vars.iter().any(named),
        K::Method(method) => method.params.iter().any(named),
        K::Lambda { params, .. } => params.iter().any(|p| p.ty.is_some() && named(p)),
        K::Class(class) => class.record_components.iter().any(named),
        _ => false,
    };
    let mut found = here;
    tree.for_each_child(&mut |child| found = found || declares(child, variable));
    found
}
