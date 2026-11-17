//! B012: an assignment inside an expression, such as `a = b = c` or `f(x = 1)` (Checkstyle InnerAssignment, PMD
//! AssignmentInOperand): the side effect is easy to overlook.
//!
//! As in Checkstyle, assignments are allowed as statements (also as the body of a control statement without braces
//! or of a rule of a `switch` statement), as lambda bodies, in the init and update parts of `for`, and as the operand
//! of a comparison in a loop condition (`while ((line = r.readLine()) != null)`), also combined with `&&`, `||`,
//! `&`, `|` and `!`. An assignment statement after a label is reported, as in Checkstyle.

use tree_sitter::Node;

use crate::lint::syntax::operator;
use crate::lint::{Diagnostic, Rule};

/// Statements whose body may be an assignment statement without braces.
const CONTROL_STATEMENTS: &[&str] =
    &["if_statement", "while_statement", "do_statement", "for_statement", "enhanced_for_statement"];

/// The parents of statements in Checkstyle's statement lists.
const STATEMENT_LISTS: &[&str] = &["block", "constructor_body", "switch_block_statement_group"];

pub fn check(assignment: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let mut ancestors = Ancestors { nodes: vec![*assignment] };
    if !is_allowed(&mut ancestors, text) {
        let offset = assignment.child_by_field_name("operator").map_or(assignment.start_byte(), |o| o.start_byte());
        diagnostics.push(Diagnostic::new(Rule::InnerAssignment, offset, "Inner assignments should be avoided"));
    }
}

/// The node and its ancestors, found as needed: `Node::parent` is expensive, and most assignments only need two.
struct Ancestors<'t> {
    nodes: Vec<Node<'t>>,
}

impl<'t> Ancestors<'t> {
    /// The node (0) or its `level`-th ancestor.
    fn get(&mut self, level: usize) -> Option<Node<'t>> {
        while self.nodes.len() <= level {
            let parent = self.nodes.last()?.parent()?;
            self.nodes.push(parent);
        }
        Some(self.nodes[level])
    }

    /// The level of the first ancestor from `level` on that is not a parenthesized expression: Checkstyle's syntax
    /// tree has no nodes for parentheses.
    fn skip_parentheses(&mut self, mut level: usize) -> usize {
        while self.get(level).is_some_and(|n| n.kind() == "parenthesized_expression") {
            level += 1;
        }
        level
    }
}

/// Whether the assignment (level 0) is in one of the contexts Checkstyle allows.
fn is_allowed(ancestors: &mut Ancestors, text: &str) -> bool {
    let parent_level = ancestors.skip_parentheses(1);
    let Some(parent) = ancestors.get(parent_level) else { return false };
    // The assignment with its parentheses.
    let Some(outermost) = ancestors.get(parent_level - 1) else { return false };
    match parent.kind() {
        "expression_statement" => ancestors.get(parent_level + 1).is_some_and(|g| {
            STATEMENT_LISTS.contains(&g.kind())
                || CONTROL_STATEMENTS.contains(&g.kind())
                || g.kind() == "switch_rule" && is_rule_of_switch_statement(ancestors, parent_level + 1)
        }),
        "lambda_expression" => true,
        "for_statement" => {
            let is_field = |name| parent.children_by_field_name(name, &mut parent.walk()).any(|c| c == outermost);
            is_field("init") || is_field("update")
        }
        "binary_expression" => {
            matches!(operator(&parent, text), "==" | "!=" | "<" | "<=" | ">" | ">=")
                && is_loop_condition(ancestors, parent_level, text)
        }
        _ => false,
    }
}

/// Whether the switch rule at the level belongs to a switch statement.
fn is_rule_of_switch_statement(ancestors: &mut Ancestors, rule_level: usize) -> bool {
    // switch_rule, switch_block, switch_expression, statement list
    ancestors.get(rule_level + 2).is_some_and(|s| s.kind() == "switch_expression")
        && ancestors.get(rule_level + 3).is_some_and(|l| STATEMENT_LISTS.contains(&l.kind()))
}

/// Whether the comparison at the level is the condition of a loop, possibly combined by `&&`, `||`, `&`, `|` and
/// `!`.
fn is_loop_condition(ancestors: &mut Ancestors, comparison_level: usize, text: &str) -> bool {
    let mut level = comparison_level + 1;
    while let Some(ancestor) = ancestors.get(level) {
        let is_logical = match ancestor.kind() {
            "parenthesized_expression" => true,
            "binary_expression" => matches!(operator(&ancestor, text), "&&" | "||" | "&" | "|"),
            "unary_expression" => operator(&ancestor, text) == "!",
            _ => false,
        };
        if !is_logical {
            return match ancestor.kind() {
                "while_statement" | "do_statement" => true,
                "for_statement" => ancestor.child_by_field_name("condition") == ancestors.get(level - 1),
                _ => false,
            };
        }
        level += 1;
    }
    false
}
