//! C003: an expression with too many boolean operators (Checkstyle BooleanExpressionComplexity).
//!
//! As in Checkstyle, counts `&&`, `||`, `&`, `|` and `^` (`&`, `|` and `^` also on numbers, and not when they are
//! the outermost operator of a method argument) in each expression of a method or constructor other than `equals`.
//! An expression is what Checkstyle's syntax tree wraps in an EXPR node: an expression statement, a condition, a
//! return value, a variable initializer, a method argument, an index, a lambda's expression body, and so on;
//! operators in a nested expression (an argument of a call within the expression) count for that one only. Code
//! outside methods (field initializers, initializer blocks) is not checked.
//!
//! The violation is reported where Checkstyle reports it, on the expression's parent in its syntax tree: for an
//! expression statement that is the opening brace of the enclosing block, for a method argument the first argument.

use std::ptr;

use super::{is_annotation_type, is_constructor_call, token_after, token_before};
use crate::ast::{Case, K, Tree};
use crate::lint::tree::{Context, has_modifier};
use crate::lint::{Diagnostic, Rule};

pub struct BooleanExpression {
    max: u32,
    /// The expressions with counted operators, in the order of their first operator: (expression address, position,
    /// count).
    expressions: Vec<(usize, usize, u32)>,
}

impl BooleanExpression {
    pub fn new(max: u32) -> BooleanExpression {
        BooleanExpression { max, expressions: Vec::new() }
    }

    pub fn enter(&mut self, tree: &Tree, ancestors: &[&Tree], context: &Context) {
        if !is_counted_operator(tree, ancestors) || !is_checked(ancestors, context) {
            return;
        }
        let Some((id, position)) = enclosing_expression(tree, ancestors, context) else { return };
        match self.expressions.iter_mut().find(|e| e.0 == id) {
            Some(expression) => expression.2 += 1,
            None => self.expressions.push((id, position, 1)),
        }
    }

    pub fn finish(self) -> Vec<Diagnostic> {
        let max = self.max;
        let too_complex = self.expressions.into_iter().filter(|e| e.2 > max);
        too_complex
            .map(|(_, position, count)| {
                let message = format!("Boolean expression complexity is {count} (max allowed is {max}).");
                Diagnostic::new(Rule::BooleanExpressionComplexity, position, message)
            })
            .collect()
    }
}

/// The innermost expression that holds the operator (or is it), as its address and where it is reported.
fn enclosing_expression(operator: &Tree, ancestors: &[&Tree], context: &Context) -> Option<(usize, usize)> {
    let mut node = operator;
    for i in (0..ancestors.len()).rev() {
        if let Some(position) = expression_position(node, &ancestors[..=i], context) {
            return Some((ptr::from_ref(node) as usize, position));
        }
        node = ancestors[i];
    }
    None
}

/// Whether code with these ancestors is checked: it must be in a method or constructor, and the innermost one must
/// not be an `equals` method (whose body typically chains comparisons of all fields).
fn is_checked(ancestors: &[&Tree], context: &Context) -> bool {
    let method = (1..ancestors.len()).rev().find_map(|i| match &ancestors[i].kind {
        K::Method(m) if !is_annotation_type(ancestors[i - 1]) => Some(m),
        _ => None,
    });
    method.is_some_and(|m| {
        let is_equals = m.ret.is_some()
            && m.name == "equals"
            && m.params.len() == 1
            && !has_modifier(&m.mods, context.text, context.tokens(), "static");
        !is_equals
    })
}

fn is_counted_operator(tree: &Tree, ancestors: &[&Tree]) -> bool {
    let K::Binary { op, .. } = &tree.kind else { return false };
    match op.as_str() {
        "&&" | "||" => true,
        "&" | "|" | "^" => {
            // Checkstyle ignores the outermost operator of a method argument.
            let mut child = tree;
            for parent in ancestors.iter().rev() {
                if !matches!(parent.kind, K::Parens(_)) {
                    return !is_argument(parent, child);
                }
                child = parent;
            }
            true
        }
        _ => false,
    }
}

/// Whether `child` is an argument of a method or constructor call, an object creation or an enum constant.
fn is_argument(parent: &Tree, child: &Tree) -> bool {
    let contains = |args: &[Tree]| args.iter().any(|a| ptr::eq(a, child));
    match &parent.kind {
        K::MethodInvocation { args, .. } | K::NewClass { args, .. } => contains(args),
        K::Class(c) => c.enum_constants.iter().any(|e| contains(&e.args)),
        _ => false,
    }
}

/// If the node is an expression that Checkstyle wraps in an EXPR node, where it reports it: on the EXPR's parent.
/// `ancestors` are the node's, the parent last.
fn expression_position(node: &Tree, ancestors: &[&Tree], context: &Context) -> Option<usize> {
    let (parent, rest) = ancestors.split_last()?;
    let grandparent = rest.last();
    let is = |t: &Tree| ptr::eq(t, node);
    match &parent.kind {
        K::ExprStmt(expression) if !is_constructor_call(expression) => {
            statement_position(parent, grandparent?, context)
        }
        K::Parens(_) => match &grandparent?.kind {
            K::If { cond, .. } | K::While { cond, .. } | K::DoWhile { cond, .. } => {
                ptr::eq(&**cond, *parent).then_some(grandparent?.start)
            }
            K::Switch { selector, .. } | K::SwitchExpr { selector, .. } => {
                ptr::eq(&**selector, *parent).then_some(grandparent?.start)
            }
            K::Synchronized { lock, .. } => ptr::eq(&**lock, *parent).then_some(grandparent?.start),
            _ => None,
        },
        K::Return(_) | K::Throw(_) | K::Yield(_) | K::Assert { .. } => Some(parent.start),
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => case_position(node, cases, context),
        K::Vars(vars) => vars.iter().any(|v| v.init.as_deref().is_some_and(is)).then(|| equals_sign(node, context)),
        K::Var(var) => var.init.as_deref().is_some_and(is).then(|| equals_sign(node, context)),
        K::MethodInvocation { args, .. } | K::NewClass { args, .. } => arguments_position(node, args, context),
        K::Class(c) => c.enum_constants.iter().find_map(|e| arguments_position(node, &e.args, context)),
        K::ArrayAccess { index, .. } => is(index).then(|| token_before(context, node.start, node.start)),
        K::NewArray { elem, dims, dim_annotations, init } => {
            if let Some(i) = dims.iter().position(is) {
                let annotation = dim_annotations.get(i).and_then(|a| a.first());
                return Some(annotation.map_or_else(|| token_before(context, node.start, node.start), |a| a.start));
            }
            // The elements of an initializer, but not the nested initializers. An array in an annotation is an element
            // value, whose elements are not expressions of their own.
            let init = init.as_deref()?;
            let is_element = init.iter().any(is)
                && !matches!(node.kind, K::NewArray { elem: None, .. })
                && !rest.iter().any(|a| matches!(a.kind, K::Annotation { .. }));
            // At the `{`.
            is_element.then(|| match elem {
                Some(_) => token_before(context, init[0].start, parent.start),
                None => parent.start,
            })
        }
        K::For { cond, update, .. } => {
            if cond.as_deref().is_some_and(is) {
                Some(root_position(node, context))
            } else if update.iter().any(is) {
                Some(root_position(&update[0], context))
            } else {
                None
            }
        }
        K::ForEach { expr, .. } => is(expr).then(|| {
            // The first token of the variable's declaration, after `for (`.
            let open = context.tokens().next(parent.start + 1).map_or(parent.start, |t| t.end);
            token_after(context, open, parent.start)
        }),
        K::Lambda { body, .. } => {
            (is(body) && !matches!(body.kind, K::Block { .. })).then(|| token_before(context, body.start, parent.start))
        }
        K::Assign { rhs, .. } if grandparent.is_some_and(|g| matches!(g.kind, K::Annotation { .. })) => {
            is(rhs).then_some(parent.start)
        }
        K::Annotation { args, .. } => {
            (args.iter().any(is) && !matches!(node.kind, K::Assign { .. })).then_some(parent.start)
        }
        _ => None,
    }
}

/// Where Checkstyle reports an expression statement: on the statement's parent.
fn statement_position(statement: &Tree, parent: &Tree, context: &Context) -> Option<usize> {
    Some(match &parent.kind {
        K::Switch { cases, .. } | K::SwitchExpr { cases, .. } => {
            // On the statement list of the `case` group, at its first statement.
            let case = cases.iter().find(|c| c.stmts.iter().any(|s| ptr::eq(s, statement)))?;
            let first = &case.stmts[0];
            match &first.kind {
                K::ExprStmt(expression) => root_position(expression, context),
                _ => first.start,
            }
        }
        // `else`.
        K::If { else_: Some(alternative), .. } if ptr::eq(&**alternative, statement) => {
            token_before(context, statement.start, parent.start)
        }
        // The `:` of the label.
        K::Labeled { .. } => token_before(context, statement.start, parent.start),
        // A `for` loop's initialization.
        K::For { init, .. } if init.iter().any(|i| ptr::eq(i, statement)) => match &init[0].kind {
            K::ExprStmt(first) => root_position(first, context),
            _ => return None,
        },
        // The `{` after `static`.
        K::Block { is_static: true, .. } => token_after(context, parent.start + 1, parent.start),
        _ => parent.start,
    })
}

/// Where Checkstyle reports the expression if it is a part of one of the `case`s: a label, a guard or a switch
/// rule's expression.
fn case_position(node: &Tree, cases: &[Case], context: &Context) -> Option<usize> {
    let is = |t: &Tree| ptr::eq(t, node);
    cases.iter().find_map(|case| {
        if case.labels.iter().any(is) {
            let is_pattern = matches!(
                node.kind,
                K::BindingPattern { .. } | K::RecordPattern { .. } | K::AnyPattern | K::DefaultCaseLabel
            );
            (!is_pattern).then_some(case.start)
        } else if case.guard.as_deref().is_some_and(is) {
            // At `when`.
            Some(token_before(context, node.start, case.start))
        } else if case.body.as_deref().is_some_and(is) {
            (!matches!(node.kind, K::Block { .. } | K::Throw(_))).then_some(case.start)
        } else {
            None
        }
    })
}

/// The position of the `=` before a variable's initializer.
fn equals_sign(initializer: &Tree, context: &Context) -> usize {
    token_before(context, initializer.start, initializer.start)
}

/// If the node is one of the arguments (but not a lambda), the position of the root of the first argument.
fn arguments_position(node: &Tree, args: &[Tree], context: &Context) -> Option<usize> {
    let is_argument = args.iter().any(|a| ptr::eq(a, node)) && !matches!(node.kind, K::Lambda { .. });
    is_argument.then(|| root_position(&args[0], context))
}

/// The position of the token at the root of Checkstyle's syntax tree of an expression, such as the operator of a
/// binary expression or the opening parenthesis of a method call.
fn root_position(expression: &Tree, context: &Context) -> usize {
    let after = |tree: &Tree| token_after(context, tree.end, expression.start);
    match &expression.kind {
        K::Binary { lhs, .. } | K::Assign { lhs, .. } => after(lhs),
        K::Conditional { cond, .. } => after(cond),
        K::MethodInvocation { select, .. } => after(select),
        K::MemberSelect { expr, .. } | K::ArrayAccess { expr, .. } | K::InstanceOf { expr, .. } => after(expr),
        K::Unary { postfix: true, expr, .. } => after(expr),
        K::MemberRef { qual, .. } => after(qual),
        K::Lambda { body, .. } => token_before(context, body.start, expression.start),
        // `new`, after `outer.`.
        K::NewClass { outer: Some(outer), .. } => {
            let dot = context.tokens().next(outer.end).map_or(outer.end, |t| t.end);
            token_after(context, dot, expression.start)
        }
        _ => expression.start,
    }
}
