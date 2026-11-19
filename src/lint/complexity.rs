//! Rules of group C: complexity and size, with Checkstyle's default limits unless `jtk.toml` sets others.
//!
//! Most of these metrics follow Checkstyle's own syntax tree, which differs from ours (for example, an `else` is a
//! node holding the alternative, consecutive `case` labels share one group, and a switch rule's arrow is a lambda
//! token), so the rules with state emulate Checkstyle's visits of the tokens they count, in one walk over the
//! compilation unit that they share (see [`Shared`]).

mod boolean_expression;
mod counts;
mod cyclomatic;
mod executable_statements;
mod file_text;
mod lengths;
mod ncss;
mod nesting_depth;

mod cognitive;

use std::cell::RefCell;

use super::tree::{Context, TreeCheck};
use super::{Diagnostic, Rule, RuleOptions};
use crate::ast::{ClassKind, K, Tag, Tree};

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[Tag], TreeCheck)] = &[
    (Rule::CyclomaticComplexity, &[Tag::CompilationUnit], shared::<{ Shared::Cyclomatic as u8 }>),
    (Rule::BooleanExpressionComplexity, &[Tag::CompilationUnit], shared::<{ Shared::BooleanExpression as u8 }>),
    (Rule::JavaNcss, &[Tag::CompilationUnit], shared::<{ Shared::Ncss as u8 }>),
    (Rule::ExecutableStatementCount, &[Tag::CompilationUnit], shared::<{ Shared::ExecutableStatements as u8 }>),
    (Rule::MethodLength, &[Tag::Method], lengths::check_method),
    (Rule::FileLength, &[Tag::CompilationUnit], file_text::check_file_length),
    (Rule::LambdaBodyLength, &[Tag::Lambda], lengths::check_lambda),
    (Rule::AnonInnerLength, &[Tag::NewClass], lengths::check_anonymous_class),
    (Rule::ParameterNumber, &[Tag::Method], counts::check_parameters),
    (Rule::RecordComponentNumber, &[Tag::Class], counts::check_record_components),
    (Rule::OuterTypeNumber, &[Tag::CompilationUnit], counts::check_outer_types),
    (Rule::NestedIfDepth, &[Tag::CompilationUnit], shared::<{ Shared::NestedIf as u8 }>),
    (Rule::NestedForDepth, &[Tag::CompilationUnit], shared::<{ Shared::NestedFor as u8 }>),
    (Rule::NestedTryDepth, &[Tag::CompilationUnit], shared::<{ Shared::NestedTry as u8 }>),
    (Rule::ThrowsCount, &[Tag::Method], counts::check_throws),
    (Rule::CognitiveComplexity, &[Tag::Method], cognitive::check),
];

/// The rules that keep state while visiting the whole compilation unit. They share one walk over the syntax tree
/// (the walk costs more than most rules' work), whose results are kept for the other rules.
#[derive(Clone, Copy)]
enum Shared {
    Cyclomatic,
    BooleanExpression,
    Ncss,
    ExecutableStatements,
    NestedIf,
    NestedFor,
    NestedTry,
}

/// The state of the shared rules during the walk.
struct Metrics<'t> {
    cyclomatic: cyclomatic::Cyclomatic,
    boolean_expression: boolean_expression::BooleanExpression,
    ncss: ncss::Ncss,
    executable_statements: executable_statements::ExecutableStatements,
    nested_if: nesting_depth::NestingDepth,
    nested_for: nesting_depth::NestingDepth,
    nested_try: nesting_depth::NestingDepth,
    /// The trees enclosing the visited one, outermost first.
    ancestors: Vec<&'t Tree>,
}

impl<'t> Metrics<'t> {
    fn new(options: &RuleOptions) -> Metrics<'t> {
        Metrics {
            cyclomatic: cyclomatic::Cyclomatic::new(u64::from(options.cyclomatic_complexity.max)),
            boolean_expression: boolean_expression::BooleanExpression::new(options.boolean_expression_complexity.max),
            ncss: ncss::Ncss::new(options.java_ncss),
            executable_statements: executable_statements::ExecutableStatements::new(
                options.executable_statement_count.max,
            ),
            nested_if: nesting_depth::NestingDepth::nested_if(options.nested_if_depth.max as usize),
            nested_for: nesting_depth::NestingDepth::nested_for(options.nested_for_depth.max as usize),
            nested_try: nesting_depth::NestingDepth::nested_try(options.nested_try_depth.max as usize),
            ancestors: Vec::new(),
        }
    }

    /// Visits the tree and its descendants in document order.
    fn walk(&mut self, tree: &'t Tree, context: &Context<'t>) {
        let ancestors = &self.ancestors;
        self.cyclomatic.enter(tree, ancestors);
        self.boolean_expression.enter(tree, ancestors, context);
        self.ncss.enter(tree, ancestors, context);
        self.executable_statements.enter(tree, ancestors);
        self.nested_if.enter(tree, ancestors);
        self.nested_for.enter(tree, ancestors);
        self.nested_try.enter(tree, ancestors);
        self.ancestors.push(tree);
        tree.for_each_child(&mut |child| self.walk(child, context));
        self.ancestors.pop();
        let ancestors = &self.ancestors;
        self.cyclomatic.leave(tree, ancestors);
        self.ncss.leave(tree, ancestors);
        self.executable_statements.leave(tree, ancestors, context);
        self.nested_if.leave(tree, ancestors);
        self.nested_for.leave(tree, ancestors);
        self.nested_try.leave(tree, ancestors);
    }

    /// The violations of each shared rule, by [`Shared`] index.
    fn finish(self, context: &Context) -> Vec<Vec<Diagnostic>> {
        vec![
            self.cyclomatic.diagnostics,
            self.boolean_expression.finish(),
            self.ncss.finish(context),
            self.executable_statements.diagnostics,
            self.nested_if.diagnostics,
            self.nested_for.diagnostics,
            self.nested_try.diagnostics,
        ]
    }
}

/// The result of the last shared walk of this thread: the address of the compilation unit, its text, the rules'
/// parameters, and the violations of each shared rule.
type SharedResult = (usize, String, RuleOptions, Vec<Vec<Diagnostic>>);

thread_local! {
    static SHARED_RESULT: RefCell<Option<SharedResult>> = const { RefCell::new(None) };
}

/// The check of a shared rule (the index of a [`Shared`]): walks the compilation unit for all shared rules, unless
/// this was already done for this compilation unit.
fn shared<'t, const RULE: u8>(unit: &'t Tree, context: &Context<'t>, diagnostics: &mut Vec<Diagnostic>) {
    let id = std::ptr::from_ref(unit) as usize;
    SHARED_RESULT.with_borrow_mut(|result| {
        let is_current =
            result.as_ref().is_some_and(|(i, t, o, _)| *i == id && t == context.text && o == context.options);
        if !is_current {
            let mut metrics = Metrics::new(context.options);
            metrics.walk(unit, context);
            let found = metrics.finish(context);
            *result = Some((id, context.text.to_string(), context.options.clone(), found));
        }
        if let Some((_, _, _, found)) = result {
            diagnostics.extend(found[usize::from(RULE)].iter().cloned());
        }
    });
}

/// Whether the tree is what Checkstyle calls a method: a method, constructor, compact constructor, static or
/// instance initializer (METHOD_DEF, CTOR_DEF, COMPACT_CTOR_DEF, STATIC_INIT, INSTANCE_INIT), but not an element of
/// an annotation type.
fn is_method_like(tree: &Tree, parent: Option<&Tree>) -> bool {
    match tree.kind {
        K::Method(_) => !parent.is_some_and(is_annotation_type),
        K::Block { .. } => parent.is_some_and(|p| matches!(p.kind, K::Class(_) | K::NewClass { .. })),
        _ => false,
    }
}

fn is_annotation_type(tree: &Tree) -> bool {
    matches!(&tree.kind, K::Class(c) if c.kind == ClassKind::Annotation)
}

/// Whether the expression is an explicit constructor invocation: `this(...)`, `super(...)` or `outer.super(...)`.
fn is_constructor_call(expression: &Tree) -> bool {
    let K::MethodInvocation { select, .. } = &expression.kind else { return false };
    match &select.kind {
        K::Ident(name) => name == "this" || name == "super",
        K::MemberSelect { name, .. } => name == "super",
        _ => false,
    }
}

/// The position of the first top-level declaration, where Checkstyle reports violations of a whole file.
fn file_position(context: &Context) -> usize {
    let tokens = context.tokens();
    let mut offset = 0;
    while let Some(token) = tokens.next(offset) {
        if &context.text[token.clone()] != ";" {
            return token.start;
        }
        offset = token.end;
    }
    0
}

/// The start of the token before the offset (skipping comments), or `fallback`.
fn token_before(context: &Context, offset: usize, fallback: usize) -> usize {
    context.tokens().previous(offset).map_or(fallback, |t| t.start)
}

/// The start of the first token at or after the offset (skipping comments), or `fallback`.
fn token_after(context: &Context, offset: usize, fallback: usize) -> usize {
    context.tokens().next(offset).map_or(fallback, |t| t.start)
}

/// The number of lines from the line of `start` to the line of `end`, both included. Lines end at `\n`, `\r\n` or
/// `\r`.
fn line_span(text: &str, start: usize, end: usize) -> usize {
    let bytes = &text.as_bytes()[start..end];
    let terminators =
        bytes.iter().enumerate().filter(|&(i, &b)| b == b'\n' || b == b'\r' && bytes.get(i + 1) != Some(&b'\n'));
    terminators.count() + 1
}
