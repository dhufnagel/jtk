//! M010: `catch` clause with the same body as an earlier one of the same `try` (PMD IdenticalCatchBranches, Eclipse
//! quick fix "Combine catch blocks"), Java 7: a multi-catch says it once.
//!
//! As in PMD, bodies are compared token by token, with the earlier clause's parameter name standing for the later
//! one's; comments do not count. PMD also requires the calls to resolve to the same methods, which needs types:
//! instead, bodies that pass the parameter to a method, or call a method on it, that is declared more than once in
//! the file (overloads, or methods of the exception classes) are taken to differ.
//!
//! Unsafe fix, for a clause identical to the one directly before it with the same parameter name: the types join
//! the earlier clause (`catch (A | B e)`) and the clause is removed. Not if the body assigns the parameter (a
//! multi-catch parameter is final), or with comments in the removed clause. Unsafe because a multi-catch of types of
//! which one extends the other does not compile, and types are needed to know.

use std::ops::Range;

use crate::lint::tree::{Context, Tokens, for_each_descendant};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{ClassKind, K, Tree};

use super::shared_b::for_each_variable_name;

pub fn check(statement: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let K::Try { catches, .. } = &statement.kind else { return };
    if catches.len() < 2 {
        return;
    }
    let (text, tokens) = (context.text, context.tokens());
    let clauses: Vec<Catch> = catches.iter().filter_map(|c| Catch::new(c, context)).collect();
    // The methods declared more than once in the file, looked up when needed.
    let mut overloaded: Option<Vec<&str>> = None;
    let mut is_overloaded = |method: &str| {
        let methods = overloaded.get_or_insert_with(|| overloaded_methods(context.ancestors[0]));
        methods.contains(&method)
    };
    // The equivalence classes, each the indexes of its clauses in order.
    let mut classes: Vec<Vec<usize>> = Vec::new();
    for (index, clause) in clauses.iter().enumerate() {
        let same = |class: &&mut Vec<usize>| clauses[class[0]].same_body(clause, text);
        let class = classes.iter_mut().find(same).filter(|_| !clause.called_methods.iter().any(|m| is_overloaded(m)));
        match class {
            Some(class) => class.push(index),
            None => classes.push(vec![index]),
        }
    }
    for class in classes {
        let first = &clauses[class[0]];
        for pair in class.windows(2) {
            let (previous, clause) = (&clauses[pair[0]], &clauses[pair[1]]);
            let message = format!("'catch' branch identical to '{}' branch", &text[first.types.clone()]);
            let diagnostic = Diagnostic::new(Rule::IdenticalCatch, clause.range.start, message);
            let fixable = pair[1] == pair[0] + 1
                && previous.name == clause.name
                && !tokens.has_comment(clause.range.clone())
                && !text[previous.range.end..clause.range.start].contains("/")
                && !assigns(clause.body, clause.name);
            diagnostics.push(if fixable {
                let types = format!("{} | {}", &text[previous.types.clone()], &text[clause.types.clone()]);
                diagnostic.with_unsafe_fix(vec![
                    Edit::new(previous.types.clone(), types),
                    Edit::new(previous.range.end..clause.range.end, ""),
                ])
            } else {
                diagnostic
            });
        }
    }
}

struct Catch<'t> {
    /// From `catch` to the end of the body.
    range: Range<usize>,
    name: &'t str,
    /// The range of the caught types (`A | B`).
    types: Range<usize>,
    body: &'t Tree,
    /// The body's tokens without comments, each with whether it is a reference to the parameter.
    tokens: Vec<(Range<usize>, bool)>,
    /// The methods called with the parameter as receiver or argument.
    called_methods: Vec<&'t str>,
}

impl<'t> Catch<'t> {
    fn new(catch: &'t jtk_parser::ast::Catch, context: &Context) -> Option<Catch<'t>> {
        let name = catch.param.name.as_str();
        let types = catch.param.ty.as_deref()?;
        let body = &*catch.block;
        let (mut references, mut called_methods) = (Vec::new(), Vec::new());
        for_each_variable_name(body, &mut |reference, ancestors| {
            if !matches!(&reference.kind, K::Ident(n) if n == name) {
                return;
            }
            references.push(reference.start);
            called_methods.extend(called_method(reference, ancestors));
        });
        Some(Catch {
            range: catch.start..body.end,
            name,
            types: types.start..types.end,
            body,
            tokens: body_tokens(body, &references, context.tokens()),
            called_methods,
        })
    }

    /// Whether the bodies are the same token by token, with the parameter names standing for each other.
    fn same_body(&self, other: &Catch, text: &str) -> bool {
        self.tokens.len() == other.tokens.len()
            && self.tokens.iter().zip(&other.tokens).all(|((a, a_reference), (b, b_reference))| {
                a_reference == b_reference && (*a_reference || text[a.clone()] == text[b.clone()])
            })
    }
}

fn body_tokens(body: &Tree, references: &[usize], tokens: &Tokens) -> Vec<(Range<usize>, bool)> {
    tokens.in_range(body.start..body.end).map(|t| (t.clone(), references.contains(&t.start))).collect()
}

/// The method that a reference to the parameter is the receiver (`e.m()`) or an argument (`m(e)`) of.
fn called_method<'t>(reference: &Tree, ancestors: &[&'t Tree]) -> Option<&'t str> {
    let (&parent, rest) = ancestors.split_last()?;
    let select = match &parent.kind {
        K::MethodInvocation { select, args, .. } if args.iter().any(|a| std::ptr::eq(a, reference)) => select,
        K::MemberSelect { name, .. } => {
            let is_receiver = rest.last().is_some_and(
                |g| matches!(&g.kind, K::MethodInvocation { select, .. } if std::ptr::eq(&**select, parent)),
            );
            return is_receiver.then_some(name.as_str());
        }
        _ => return None,
    };
    match &select.kind {
        K::Ident(name) | K::MemberSelect { name, .. } => Some(name),
        _ => None,
    }
}

/// The names of the methods declared more than once.
fn overloaded_methods(unit: &Tree) -> Vec<&str> {
    let mut declared = std::collections::HashSet::new();
    let mut overloaded = Vec::new();
    for_each_descendant(unit, &mut |tree, parent| {
        let in_annotation_type = matches!(&parent.kind, K::Class(c) if c.kind == ClassKind::Annotation);
        if let K::Method(method) = &tree.kind
            && method.ret.is_some()
            && !in_annotation_type
            && !declared.insert(method.name.as_str())
        {
            overloaded.push(method.name.as_str());
        }
        true
    });
    overloaded
}

/// Whether the body assigns the variable (`e = ...`, `e++`).
fn assigns(body: &Tree, name: &str) -> bool {
    let mut found = false;
    for_each_descendant(body, &mut |tree, _| {
        let target = match &tree.kind {
            K::Assign { lhs, .. } => Some(lhs),
            K::Unary { op, expr, .. } if op == "++" || op == "--" => Some(expr),
            _ => None,
        };
        found |= target.is_some_and(|t| matches!(&t.kind, K::Ident(n) if n == name));
        !found
    });
    found
}
