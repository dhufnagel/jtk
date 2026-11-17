//! M003: a pattern `case ... ->` whose block only holds an `if` without `else` (Checkstyle WhenShouldBeUsed),
//! Java 21: the condition can be a `when` guard. As in Checkstyle, the block may also hold `break`, empty statements
//! and `if` statements with `else`, as long as exactly one `if` has no `else`.
//!
//! The fix only applies where the guard does not change which case runs: the block is just the `if`, the case has no
//! guard yet, and the only case after it is a `default` with an empty block. Elsewhere a false guard would go on to
//! the next cases instead of doing nothing. The condition may only name the pattern variables: a guard cannot use
//! local variables that are not effectively final.

use tree_sitter::Node;

use crate::lint::syntax::{
    for_each_descendant, for_each_descendant_with_parent, is_comment, is_name_reference_in, named_children, source,
    strip_parentheses,
};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(rule: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let children = named_children(rule);
    let (Some(label), Some(body)) = (children.first(), children.get(1)) else { return };
    let label_children = named_children(label);
    if body.kind() != "block" || !label_children.iter().any(|c| c.kind() == "pattern") {
        return;
    }
    let mut cursor = body.walk();
    let statements: Vec<Node> = body.named_children(&mut cursor).filter(|s| !is_comment(s)).collect();
    if !statements.iter().all(|s| matches!(s.kind(), "if_statement" | "break_statement")) {
        return;
    }
    let without_else: Vec<&Node> = statements
        .iter()
        .filter(|s| s.kind() == "if_statement" && s.child_by_field_name("alternative").is_none())
        .collect();
    let [guarded] = without_else.as_slice() else { return };
    let message = "'when' expression should be used instead of a single 'if' statement in case block";
    let mut diagnostic = Diagnostic::new(Rule::WhenGuard, label.start_byte(), message);
    let has_guard = label_children.iter().any(|c| c.kind() == "guard");
    // Comments and empty statements next to the `if` would be lost.
    let only_the_if = body.children(&mut cursor).filter(|c| !matches!(c.kind(), "{" | "}")).count() == 1;
    let fixable = !has_guard
        && only_the_if
        && uses_only_pattern_variables(label, guarded, text)
        && is_followed_by_empty_default(rule, text);
    if fixable {
        diagnostic = diagnostic.with_fix(guard_fix(label, body, guarded, text));
    }
    diagnostics.push(diagnostic);
}

/// Whether the condition only refers to the label's pattern variables by name: a guard may only use effectively
/// final local variables, and a name could be a local variable that is not.
fn uses_only_pattern_variables(label: &Node, statement: &Node, text: &str) -> bool {
    let mut pattern_variables = Vec::new();
    for_each_descendant(label, &[], |n| {
        if n.kind() == "identifier" {
            pattern_variables.push(source(n, text));
        }
    });
    let Some(condition) = statement.child_by_field_name("condition") else { return false };
    let mut only_patterns = true;
    for_each_descendant_with_parent(&condition, |n, parent| {
        if n.kind() == "identifier" && is_name_reference_in(n, parent) {
            only_patterns &= pattern_variables.contains(&source(n, text));
        }
    });
    only_patterns
}

/// Whether the only case after `rule` is `default -> {}`.
fn is_followed_by_empty_default(rule: &Node, text: &str) -> bool {
    let mut next = rule.next_named_sibling();
    while next.is_some_and(|n| is_comment(&n)) {
        next = next.and_then(|n| n.next_named_sibling());
    }
    let Some(default) = next.filter(|n| n.kind() == "switch_rule") else { return false };
    let mut after = default.next_named_sibling();
    while after.is_some_and(|n| is_comment(&n)) {
        after = after.and_then(|n| n.next_named_sibling());
    }
    let children = named_children(&default);
    let is_default = children.first().is_some_and(|l| l.kind() == "switch_label" && source(l, text) == "default");
    let empty_block = children.get(1).is_some_and(|b| b.kind() == "block" && named_children(b).is_empty());
    after.is_none() && is_default && empty_block
}

/// `case P -> { if (c) S }` becomes `case P when c -> S'`, where `S'` is `S` as a block.
fn guard_fix(label: &Node, body: &Node, statement: &Node, text: &str) -> Vec<Edit> {
    let (Some(condition), Some(consequence)) =
        (statement.child_by_field_name("condition"), statement.child_by_field_name("consequence"))
    else {
        return Vec::new();
    };
    let inner = source(&strip_parentheses(&condition), text);
    let guard = if inner.contains("->") { source(&condition, text) } else { inner };
    let replacement = match consequence.kind() {
        "block" => source(&consequence, text).to_string(),
        _ => format!("{{ {} }}", source(&consequence, text)),
    };
    vec![
        Edit::new(label.end_byte()..label.end_byte(), format!(" when {guard}")),
        Edit::new(body.byte_range(), replacement),
    ]
}
