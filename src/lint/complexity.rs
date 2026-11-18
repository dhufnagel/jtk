//! Rules of group C: complexity and size, with Checkstyle's default limits unless `jtk.toml` sets others.
//!
//! Most of these metrics follow Checkstyle's own syntax tree, which differs from tree-sitter's (for example, an `else`
//! is a node holding the alternative, consecutive `case` labels share one group, and a switch rule's arrow is a
//! lambda token), so the rules with state emulate Checkstyle's visits of the tokens they count, in one walk over the
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
use std::collections::HashMap;

use tree_sitter::Node;

use super::syntax::is_comment;
use super::{ConfiguredCheck, Diagnostic, Rule, RuleOptions};

/// The checks on the AST, with the kinds of trees each checks.
pub(super) const TREE_CHECKS: &[(Rule, &[crate::ast::Tag], super::tree::TreeCheck)] = &[];

pub(super) const CONFIGURED_CHECKS: &[(Rule, &[&str], ConfiguredCheck)] = &[
    (Rule::CyclomaticComplexity, &["program"], shared::<{ Shared::Cyclomatic as u8 }>),
    (Rule::BooleanExpressionComplexity, &["program"], shared::<{ Shared::BooleanExpression as u8 }>),
    (Rule::JavaNcss, &["program"], shared::<{ Shared::Ncss as u8 }>),
    (Rule::ExecutableStatementCount, &["program"], shared::<{ Shared::ExecutableStatements as u8 }>),
    (
        Rule::MethodLength,
        &["method_declaration", "constructor_declaration", "compact_constructor_declaration"],
        lengths::check_method,
    ),
    (Rule::FileLength, &["program"], file_text::check_file_length),
    (Rule::LambdaBodyLength, &["lambda_expression"], lengths::check_lambda),
    (Rule::AnonInnerLength, &["object_creation_expression"], lengths::check_anonymous_class),
    (Rule::ParameterNumber, &["method_declaration", "constructor_declaration"], counts::check_parameters),
    (Rule::RecordComponentNumber, &["record_declaration"], counts::check_record_components),
    (Rule::OuterTypeNumber, &["program"], counts::check_outer_types),
    (Rule::NestedIfDepth, &["program"], shared::<{ Shared::NestedIf as u8 }>),
    (Rule::NestedForDepth, &["program"], shared::<{ Shared::NestedFor as u8 }>),
    (Rule::NestedTryDepth, &["program"], shared::<{ Shared::NestedTry as u8 }>),
    (Rule::LineLength, &["program"], file_text::check_line_length),
    (Rule::ThrowsCount, &["method_declaration", "constructor_declaration"], counts::check_throws),
    (Rule::CognitiveComplexity, &["program"], cognitive::check),
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

impl Shared {
    const ALL: [Shared; 7] = [
        Shared::Cyclomatic,
        Shared::BooleanExpression,
        Shared::Ncss,
        Shared::ExecutableStatements,
        Shared::NestedIf,
        Shared::NestedFor,
        Shared::NestedTry,
    ];

    fn visitor<'t, 'a>(
        self,
        text: &'a str,
        options: &RuleOptions,
        diagnostics: &'a mut Vec<Diagnostic>,
    ) -> Visitor<'t, 'a> {
        let o = options;
        match self {
            Shared::Cyclomatic => cyclomatic::visitor(text, u64::from(o.cyclomatic_complexity.max), diagnostics),
            Shared::BooleanExpression => {
                boolean_expression::visitor(text, o.boolean_expression_complexity.max, diagnostics)
            }
            Shared::Ncss => ncss::visitor(text, o.java_ncss, diagnostics),
            Shared::ExecutableStatements => {
                executable_statements::visitor(text, o.executable_statement_count.max, diagnostics)
            }
            Shared::NestedIf => nesting_depth::visitor_if(text, o.nested_if_depth.max as usize, diagnostics),
            Shared::NestedFor => nesting_depth::visitor_for(text, o.nested_for_depth.max as usize, diagnostics),
            Shared::NestedTry => nesting_depth::visitor_try(text, o.nested_try_depth.max as usize, diagnostics),
        }
    }
}

/// The result of the last shared walk of this thread: the id of the compilation unit, its text, the rules'
/// parameters, and the violations of each shared rule.
type SharedResult = (usize, String, RuleOptions, Vec<Vec<Diagnostic>>);

thread_local! {
    static SHARED_RESULT: RefCell<Option<SharedResult>> = const { RefCell::new(None) };
}

/// The check of a shared rule (the index of a [`Shared`]): walks the compilation unit for all shared rules, unless
/// this was already done for this compilation unit.
fn shared<const RULE: u8>(program: &Node, text: &str, options: &RuleOptions, diagnostics: &mut Vec<Diagnostic>) {
    SHARED_RESULT.with_borrow_mut(|result| {
        let is_current = result.as_ref().is_some_and(|(id, t, o, _)| *id == program.id() && t == text && o == options);
        if !is_current {
            let mut found: Vec<Vec<Diagnostic>> = vec![Vec::new(); Shared::ALL.len()];
            let mut visitors: Vec<Visitor> =
                Shared::ALL.iter().zip(found.iter_mut()).map(|(rule, d)| rule.visitor(text, options, d)).collect();
            walk(program, &mut visitors);
            drop(visitors);
            *result = Some((program.id(), text.to_string(), options.clone(), found));
        }
        if let Some((_, _, _, found)) = result {
            diagnostics.extend(found[usize::from(RULE)].iter().cloned());
        }
    });
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Event {
    Enter,
    Leave,
}

/// A set of node kinds (named or anonymous), by kind id: comparing ids is much cheaper than comparing kind names,
/// which matters for the rules that visit every node.
struct KindSet(Vec<bool>);

impl KindSet {
    fn new(kinds: &[&str]) -> KindSet {
        let language: tree_sitter::Language = tree_sitter_java_orchard::LANGUAGE.into();
        let ids = 0..language.node_kind_count() as u16;
        KindSet(ids.map(|id| language.node_kind_for_id(id).is_some_and(|kind| kinds.contains(&kind))).collect())
    }

    fn contains(&self, kind_id: u16) -> bool {
        self.0.get(usize::from(kind_id)).copied().unwrap_or(false)
    }
}

/// Declares the set of node kinds once, as a lazily created static.
macro_rules! kinds {
    ($($kind:literal),* $(,)?) => {{
        static KINDS: std::sync::OnceLock<crate::lint::complexity::KindSet> = std::sync::OnceLock::new();
        KINDS.get_or_init(|| crate::lint::complexity::KindSet::new(&[$($kind),*]))
    }};
}
use kinds;

/// A rule's visit of the nodes of the given kinds, called when entering and when leaving each of them with its
/// ancestors (the parent last).
type Visitor<'t, 'a> = (&'static KindSet, Box<dyn FnMut(Event, &Node<'t>, &[Node<'t>]) + 'a>);

/// Visits the nodes in document order: calls each visitor for the nodes of its kinds. Iterative, so that deeply
/// nested code cannot overflow the stack.
fn walk<'t>(root: &Node<'t>, visitors: &mut [Visitor<'t, '_>]) {
    let mut cursor = root.walk();
    let mut ancestors: Vec<Node<'t>> = Vec::new();
    let mut visit = |event, node: &Node<'t>, ancestors: &[Node<'t>]| {
        let kind = node.kind_id();
        for (kinds, visitor) in visitors.iter_mut() {
            if kinds.contains(kind) {
                visitor(event, node, ancestors);
            }
        }
    };
    visit(Event::Enter, root, &ancestors);
    loop {
        let node = cursor.node();
        if cursor.goto_first_child() {
            ancestors.push(node);
            visit(Event::Enter, &cursor.node(), &ancestors);
            continue;
        }
        loop {
            visit(Event::Leave, &cursor.node(), &ancestors);
            if ancestors.is_empty() {
                return;
            }
            if cursor.goto_next_sibling() {
                visit(Event::Enter, &cursor.node(), &ancestors);
                break;
            }
            cursor.goto_parent();
            ancestors.pop();
        }
    }
}

/// Whether the node is what Checkstyle calls a method: a method, constructor, compact constructor, static or
/// instance initializer (METHOD_DEF, CTOR_DEF, COMPACT_CTOR_DEF, STATIC_INIT, INSTANCE_INIT).
fn is_method_like(node: &Node, parent: Option<&Node>) -> bool {
    match node.kind() {
        "method_declaration" | "constructor_declaration" | "compact_constructor_declaration" | "static_initializer" => {
            true
        }
        "block" => is_instance_initializer(node, parent),
        _ => false,
    }
}

/// Whether the block is an instance initializer (a block directly in a class body).
fn is_instance_initializer(block: &Node, parent: Option<&Node>) -> bool {
    block.kind() == "block" && parent.is_some_and(|p| matches!(p.kind(), "class_body" | "enum_body_declarations"))
}

fn is_type_declaration(node: &Node) -> bool {
    matches!(
        node.kind(),
        "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "annotation_type_declaration"
            | "record_declaration"
    )
}

/// The children of a node without comments.
fn code_children<'t>(node: &Node<'t>) -> Vec<Node<'t>> {
    node.children(&mut node.walk()).filter(|c| !is_comment(c)).collect()
}

/// The first anonymous child token of the given kind.
fn token<'t>(node: &Node<'t>, kind: &str) -> Option<Node<'t>> {
    node.children(&mut node.walk()).find(|c| !c.is_named() && c.kind() == kind)
}

/// The 1-based line where the node starts.
fn line(node: &Node) -> usize {
    node.start_position().row + 1
}

/// The 1-based line of the node's last character.
fn end_line(node: &Node) -> usize {
    let end = node.end_position();
    if end.column == 0 && end.row > node.start_position().row { end.row } else { end.row + 1 }
}

/// The position of the first top-level node, where Checkstyle reports violations of a whole file.
fn file_position(program: &Node) -> usize {
    let first = program.named_children(&mut program.walk()).find(|c| !is_comment(c));
    first.map_or(0, |c| c.start_byte())
}

/// Whether a `switch` group has no statements, only labels.
fn is_label_only_group(group: &Node) -> bool {
    group.named_children(&mut group.walk()).all(|c| c.kind() == "switch_label" || is_comment(&c))
}

/// Whether a switch label starts with `case` (rather than `default`).
fn is_case_label(label: &Node) -> bool {
    label.child(0).is_some_and(|c| c.kind() == "case")
}

/// Where Checkstyle's groups of a `switch` block (CASE_GROUP) start and end: tree-sitter gives each run of labels
/// without statements a group of its own, Checkstyle merges it with the following group.
#[derive(Default)]
struct CaseGroups(HashMap<usize, CaseGroup>);

#[derive(Clone, Copy)]
struct CaseGroup {
    starts: bool,
    ends: bool,
    /// The number of `case` labels of the Checkstyle group that the tree-sitter group starts.
    cases: usize,
}

impl CaseGroups {
    /// Adds the groups of a switch block, which must be added before its groups are visited.
    fn add(&mut self, switch_block: &Node) {
        let groups: Vec<Node> = switch_block
            .named_children(&mut switch_block.walk())
            .filter(|c| c.kind() == "switch_block_statement_group")
            .collect();
        let mut start = 0;
        for (i, group) in groups.iter().enumerate() {
            let ends = !is_label_only_group(group) || i + 1 == groups.len();
            self.0.insert(group.id(), CaseGroup { starts: start == i, ends, cases: 0 });
            if ends {
                let cases = groups[start..=i]
                    .iter()
                    .flat_map(|g| g.named_children(&mut g.walk()).collect::<Vec<_>>())
                    .filter(|c| c.kind() == "switch_label" && is_case_label(c))
                    .count();
                if let Some(first) = self.0.get_mut(&groups[start].id()) {
                    first.cases = cases;
                }
                start = i + 1;
            }
        }
    }

    fn get(&self, group: &Node) -> CaseGroup {
        self.0.get(&group.id()).copied().unwrap_or(CaseGroup { starts: true, ends: true, cases: 0 })
    }
}
