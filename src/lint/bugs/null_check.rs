//! B016: null checks that come too late or test the wrong way (PMD MisplacedNullCheck, BrokenNullCheck):
//! `x.foo() && x != null` dereferences before checking, `x != null || x.foo()` dereferences exactly when `x` is null.
//!
//! Like PMD, the checked expression may be a variable, a field or a getter call (`a.getB()`), and besides calls,
//! field and array accesses, method references (`x::foo`), qualified instance creations (`x.new Inner()`) and
//! unboxing (`x == 0`, `x < 1`, `x && b`) count as uses that throw. A use guarded by its own null check of the same
//! expression (`(x != null && x.foo()) || x == null`) is not reported. Deviations: jtk reports the null check, not
//! the use, and checks every operand of a chain of `&&` or `||`, while PMD's BrokenNullCheck only looks at the
//! operand right after the check. `x + 1` only counts as unboxing if `x` is declared with a wrapper type, as it may
//! concatenate strings.

use super::shared_b::{declared_type, for_each_in_same_code};
use crate::ast::{K, LitKind, Tree};
use crate::lint::tree::{Context, path_to, strip_parens};
use crate::lint::{Diagnostic, Rule};

pub fn check<'t>(tree: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let K::Binary { op: chain_operator, .. } = &tree.kind else { return };
    if !matches!(chain_operator.as_str(), "&&" | "||") {
        return;
    }
    let is_chain_top =
        context.parent().is_none_or(|p| !matches!(&p.kind, K::Binary { op, .. } if op == chain_operator));
    if !is_chain_top {
        return;
    }
    let mut operands = Vec::new();
    flatten(tree, chain_operator, &mut operands);
    // In `a && b`, b is evaluated when a is true; in `a || b`, when a is false.
    let continues_when_null = |is_null_test: bool| (chain_operator == "&&") == is_null_test;
    for (i, operand) in operands.iter().enumerate() {
        let Some((checked, is_null_test)) = null_test(operand) else { continue };
        let checked = Checked { tree: checked, text: context.source(checked), chain: tree, context };
        let throws = |o: &&Tree| checked.throws_if_null(o);
        let message = if operands[..i].iter().any(throws) {
            format!("'{}' is dereferenced before it is checked for null", checked.text)
        } else if continues_when_null(is_null_test) && operands[i + 1..].iter().any(throws) {
            format!("'{}' is dereferenced exactly when it is null", checked.text)
        } else {
            continue;
        };
        diagnostics.push(Diagnostic::new(Rule::NullCheck, operand.start, message));
    }
}

fn flatten<'t>(tree: &'t Tree, chain_operator: &str, operands: &mut Vec<&'t Tree>) {
    let tree = strip_parens(tree);
    match &tree.kind {
        K::Binary { op, lhs, rhs } if op == chain_operator => {
            flatten(lhs, chain_operator, operands);
            flatten(rhs, chain_operator, operands);
        }
        _ => operands.push(tree),
    }
}

/// `x == null` or `x != null` (either way round): the tested expression, and whether the test is `== null`.
fn null_test(operand: &Tree) -> Option<(&Tree, bool)> {
    let K::Binary { op, lhs, rhs } = &operand.kind else { return None };
    let is_null_test = match op.as_str() {
        "==" => true,
        "!=" => false,
        _ => return None,
    };
    let (left, right) = (strip_parens(lhs), strip_parens(rhs));
    let is_null = |t: &Tree| matches!(t.kind, K::Literal(LitKind::Null));
    let checked = if is_null(right) {
        left
    } else if is_null(left) {
        right
    } else {
        return None;
    };
    let is_this = matches!(&checked.kind, K::Ident(name) if name == "this");
    (is_stable_path(checked) && !is_this).then_some((checked, is_null_test))
}

/// A variable, or a chain of field accesses and getter calls on one: an expression that gives the same value when
/// evaluated twice in a row (PMD's `StablePathMatcher`).
fn is_stable_path(expression: &Tree) -> bool {
    match &expression.kind {
        K::Ident(name) => name != "super",
        K::MemberSelect { expr, name } => name != "class" && is_stable_path(expr),
        K::MethodInvocation { select, args, .. } => {
            let (name, object) = match &select.kind {
                K::Ident(name) => (name, None),
                K::MemberSelect { expr, name } => (name, Some(expr)),
                _ => return false,
            };
            let is_getter = ["get", "is"].iter().any(|prefix| {
                name.strip_prefix(prefix).and_then(|rest| rest.chars().next()).is_some_and(char::is_uppercase)
            });
            is_getter && args.is_empty() && object.is_none_or(|o| is_stable_path(o))
        }
        _ => false,
    }
}

/// The expression a null check tests, in the chain of the check.
struct Checked<'a, 't> {
    tree: &'t Tree,
    text: &'t str,
    chain: &'t Tree,
    context: &'a Context<'t>,
}

impl Checked<'_, '_> {
    fn is(&self, tree: &Tree) -> bool {
        self.context.source(strip_parens(tree)) == self.text
    }

    /// Whether evaluating the operand throws a NullPointerException if the checked expression is null: it
    /// dereferences or unboxes it, and does not check it for null itself.
    fn throws_if_null(&self, operand: &Tree) -> bool {
        let mut throws = self.is(operand);
        let mut guarded = false;
        let mut visit = |tree: &Tree| {
            guarded |= null_test(tree).is_some_and(|(c, _)| self.context.source(c) == self.text);
            throws = throws
                || match &tree.kind {
                    // A field access, or the method of a call.
                    K::MemberSelect { expr, name } => name != "class" && self.is(expr),
                    K::ArrayAccess { expr, .. } => self.is(expr),
                    K::MemberRef { qual, .. } => self.is(qual),
                    K::NewClass { outer: Some(outer), .. } => self.is(outer),
                    K::Unary { op, postfix: false, expr } => {
                        matches!(op.as_str(), "!" | "-" | "+" | "~") && self.is(expr)
                    }
                    K::Binary { op, lhs, rhs } => self.unboxes(op, lhs, rhs),
                    _ => false,
                };
        };
        visit(operand);
        for_each_in_same_code(operand, &mut visit);
        throws && !guarded
    }

    /// Whether the binary expression unboxes an operand that is the checked expression.
    fn unboxes(&self, op: &str, left: &Tree, right: &Tree) -> bool {
        let other = if self.is(left) {
            right
        } else if self.is(right) {
            left
        } else {
            return false;
        };
        match op {
            "==" | "!=" => self.is_primitive_literal(strip_parens(other)),
            "+" => self.is_wrapper_variable(),
            _ => true,
        }
    }

    /// A literal of a primitive type; like tree-sitter's literals, not a negative number.
    fn is_primitive_literal(&self, expression: &Tree) -> bool {
        let K::Literal(kind) = expression.kind else { return false };
        kind != LitKind::Null && kind != LitKind::String && !self.context.source(expression).starts_with('-')
    }

    /// Whether the checked expression is a variable declared with a numeric wrapper type, which `+` unboxes.
    fn is_wrapper_variable(&self) -> bool {
        let K::Ident(name) = &self.tree.kind else { return false };
        let mut path = self.context.ancestors.clone();
        path.push(self.chain);
        path.extend(path_to(self.chain, self.tree.start..self.tree.end));
        declared_type(&path, name, self.context.text)
            .is_some_and(|t| matches!(t, "Integer" | "Long" | "Short" | "Byte" | "Character" | "Double" | "Float"))
    }
}
