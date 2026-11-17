//! B004: a `case` with code that falls through to the next one (Checkstyle FallThrough, PMD
//! ImplicitSwitchFallThrough, Error Prone FallThrough). A comment like `// fall through` on the last non-empty line
//! before the next `case` marks it as intended.
//!
//! As in Checkstyle, a loop counts as ending the `case` if its body ends in `return` or `throw`, or if it is an
//! infinite loop without `break`.

use tree_sitter::Node;

use crate::lint::syntax::{NESTED_CODE, descendants, is_comment, named_children, source, strip_parentheses};
use crate::lint::{Diagnostic, Rule};

/// Checkstyle's default relief pattern `falls?[ -]?thr(u|ough)`, spelled out.
const RELIEF_COMMENTS: &[&str] = &[
    "fallthru",
    "fall thru",
    "fall-thru",
    "fallthrough",
    "fall through",
    "fall-through",
    "fallsthru",
    "falls thru",
    "falls-thru",
    "fallsthrough",
    "falls through",
    "falls-through",
];

/// Statements whose `break` does not leave the enclosing `case`.
const BREAK_TARGETS: &[&str] = &[
    "for_statement",
    "enhanced_for_statement",
    "while_statement",
    "do_statement",
    "switch_block",
    "lambda_expression",
    "class_body",
];

/// How a statement can leave the `case` (the byte range of its statement group).
#[derive(Clone, Copy)]
struct Exits {
    by_break: bool,
    by_continue: bool,
    case: (usize, usize),
}

pub fn check(switch_block: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let groups: Vec<Node> =
        named_children(switch_block).into_iter().filter(|c| c.kind() == "switch_block_statement_group").collect();
    for pair in groups.windows(2) {
        let [group, next] = pair else { continue };
        let Some(last) = statements(group).pop() else { continue };
        let next_case = first_label_start(next);
        if !ends_group(&last, group, text) && !is_relieved(&text[last.end_byte()..next_case]) {
            diagnostics.push(Diagnostic::new(Rule::FallThrough, next_case, "Fall through from the previous case"));
        }
    }
}

/// Whether control cannot reach the end of a statement of a `case` group, so that the group does not fall through:
/// a `break` or `continue` leaves the group unless it targets a statement within it.
pub(crate) fn ends_group(statement: &Node, group: &Node, text: &str) -> bool {
    terminates(
        statement,
        text,
        Exits { by_break: true, by_continue: true, case: (group.start_byte(), group.end_byte()) },
    )
}

/// Whether the last non-empty line between the case's code and the next `case` has a relief comment.
fn is_relieved(between: &str) -> bool {
    let last_line = between.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
    RELIEF_COMMENTS.iter().any(|c| last_line.contains(c))
}

fn first_label_start(group: &Node) -> usize {
    let label = named_children(group).into_iter().find(|c| c.kind() == "switch_label");
    label.map_or(group.start_byte(), |l| l.start_byte())
}

fn statements<'t>(node: &Node<'t>) -> Vec<Node<'t>> {
    named_children(node).into_iter().filter(|c| c.kind() != "switch_label" && !is_comment(c)).collect()
}

/// Whether control cannot reach the end of the statement.
fn terminates(statement: &Node, text: &str, exits: Exits) -> bool {
    let field = |name| statement.child_by_field_name(name);
    match statement.kind() {
        "return_statement" | "throw_statement" | "yield_statement" => true,
        "break_statement" => jumps_out(statement, text, exits.by_break, exits),
        "continue_statement" => jumps_out(statement, text, exits.by_continue, exits),
        "block" => statements(statement).last().is_some_and(|s| terminates(s, text, exits)),
        "if_statement" => match (field("consequence"), field("alternative")) {
            (Some(consequence), Some(alternative)) => {
                terminates(&consequence, text, exits) && terminates(&alternative, text, exits)
            }
            _ => false,
        },
        "try_statement" | "try_with_resources_statement" => try_terminates(statement, text, exits),
        "synchronized_statement" => field("body").is_some_and(|b| terminates(&b, text, exits)),
        "labeled_statement" => named_children(statement).last().is_some_and(|s| terminates(s, text, exits)),
        "while_statement" | "do_statement" | "for_statement" | "enhanced_for_statement" => {
            loop_terminates(statement, text, exits)
        }
        "switch_expression" => switch_terminates(statement, text, exits),
        "expression_statement" => named_children(statement)
            .first()
            .is_some_and(|e| e.kind() == "switch_expression" && switch_terminates(e, text, exits)),
        _ => false,
    }
}

/// Whether a `break` or `continue` leaves the case: an unlabeled one if `unlabeled_leaves`, a labeled one unless it
/// targets a statement within the case.
fn jumps_out(statement: &Node, text: &str, unlabeled_leaves: bool, exits: Exits) -> bool {
    let Some(label) = named_children(statement).into_iter().find(|c| c.kind() == "identifier") else {
        return unlabeled_leaves;
    };
    let target = std::iter::successors(statement.parent(), |n| n.parent())
        .filter(|n| n.kind() == "labeled_statement")
        .find(|l| named_children(l).first().is_some_and(|name| source(name, text) == source(&label, text)));
    let (case_start, case_end) = exits.case;
    target.is_none_or(|t| t.start_byte() < case_start || t.end_byte() > case_end)
}

fn try_terminates(statement: &Node, text: &str, exits: Exits) -> bool {
    let children = named_children(statement);
    let finally = children.iter().find(|c| c.kind() == "finally_clause");
    if finally.and_then(|f| named_children(f).pop()).is_some_and(|block| terminates(&block, text, exits)) {
        return true;
    }
    let body_terminates = statement.child_by_field_name("body").is_some_and(|b| terminates(&b, text, exits));
    let mut catch_bodies =
        children.iter().filter(|c| c.kind() == "catch_clause").filter_map(|c| c.child_by_field_name("body"));
    body_terminates && catch_bodies.all(|b| terminates(&b, text, exits))
}

fn loop_terminates(statement: &Node, text: &str, exits: Exits) -> bool {
    let Some(body) = statement.child_by_field_name("body") else { return false };
    let is_infinite = match statement.kind() {
        "for_statement" => statement.child_by_field_name("condition").is_none_or(|c| is_true(&c, text)),
        "enhanced_for_statement" => false,
        _ => statement.child_by_field_name("condition").is_some_and(|c| is_true(&c, text)),
    };
    let has_break = has_break(statement, &body, text);
    let body_exits = Exits { by_break: false, by_continue: false, ..exits };
    (is_infinite && !has_break) || terminates(&body, text, body_exits)
}

/// Whether a `break` leaves the loop: an unlabeled one outside nested loops and switches, or one naming the loop's
/// label.
fn has_break<'t>(statement: &Node<'t>, body: &Node<'t>, text: &str) -> bool {
    let labels: Vec<&str> = std::iter::successors(statement.parent(), |n| n.parent())
        .take_while(|n| n.kind() == "labeled_statement")
        .filter_map(|l| named_children(&l).first().map(|name| source(name, text)))
        .collect();
    let label_of = |b: &Node| named_children(b).first().map(|l| source(l, text));
    let breaks_outside = |opaque: &[&str]| -> Vec<Node<'t>> {
        let mut breaks = descendants(body, "break_statement", opaque);
        if body.kind() == "break_statement" {
            breaks.push(*body);
        }
        breaks
    };
    let unlabeled = breaks_outside(BREAK_TARGETS).iter().any(|b| label_of(b).is_none());
    let to_loop = breaks_outside(NESTED_CODE).iter().any(|b| label_of(b).is_some_and(|l| labels.contains(&l)));
    unlabeled || to_loop
}

/// A nested `switch` ends the case if it has a `default` and all its cases end it (a `break` only leaves the nested
/// switch).
fn switch_terminates(switch: &Node, text: &str, exits: Exits) -> bool {
    let Some(block) = switch.child_by_field_name("body") else { return false };
    let groups: Vec<Node> = named_children(&block).into_iter().filter(|g| !is_comment(g)).collect();
    let has_default = groups
        .iter()
        .flat_map(named_children)
        .any(|c| c.kind() == "switch_label" && source(&c, text).starts_with("default"));
    let nested = Exits { by_break: false, ..exits };
    has_default
        && groups.iter().all(|g| match g.kind() {
            "switch_block_statement_group" => statements(g).last().is_some_and(|s| terminates(s, text, nested)),
            _ => false,
        })
}

fn is_true(condition: &Node, text: &str) -> bool {
    source(&strip_parentheses(condition), text) == "true"
}
