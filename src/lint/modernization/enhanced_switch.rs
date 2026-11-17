//! M002: `switch` with `case ...:` groups that could use the arrow form (`case ... ->`) because no group falls
//! through (Checkstyle UseEnhancedSwitch, Eclipse save action "Convert to switch expression" in part), Java 14.
//!
//! As in Checkstyle, every group but the last must end in a statement that cannot complete normally (`return`,
//! `throw`, `yield`, `break`, `continue`, or an `if`, `try`, loop, `switch` or block ending so). Only the group's
//! last statement counts.
//!
//! Unsafe fix, for `switch` statements whose groups each end in `break` (removed), `return`, `throw` or `continue`
//! (the last group also without one), with no other `break` of the switch and no local variable used in another
//! group: each group becomes a rule (`case 1, 2 -> { ... }`). Not with comments outside the statements, `default`
//! together with other labels, or several pattern labels. Unsafe because the checks are syntactic: whether a
//! variable declared in one group is used in another is judged by its name.

use tree_sitter::Node;

use crate::lint::bugs::fall_through::ends_group;
use crate::lint::syntax::{is_comment, named_children, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(switch: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    let Some(body) = switch.child_by_field_name("body") else { return };
    let groups: Vec<Node> =
        named_children(&body).into_iter().filter(|c| c.kind() == "switch_block_statement_group").collect();
    // Labels without statements form one group with the next ones, as in Checkstyle.
    let last_statements: Vec<Node> = groups.iter().filter_map(last_statement).collect();
    let Some(last_group) = groups.last() else { return };
    let leading = match last_statement(last_group) {
        Some(_) => &last_statements[..last_statements.len() - 1],
        None => &last_statements[..],
    };
    // The statements are direct children of their groups.
    if leading.iter().all(|s| s.parent().is_some_and(|group| ends_group(s, &group, text))) {
        let message = "Switch can be replaced with enhanced switch";
        let diagnostic = Diagnostic::new(Rule::EnhancedSwitch, switch.start_byte(), message);
        diagnostics.push(match arrow_form(switch, &body, text) {
            Some(edit) => diagnostic.with_unsafe_fix(vec![edit]),
            None => diagnostic,
        });
    }
}

/// The edit that writes the body of a `switch` statement with rules, if that is simple and keeps the meaning.
fn arrow_form(switch: &Node, body: &Node, text: &str) -> Option<Edit> {
    let is_statement = switch.parent().is_some_and(|p| {
        matches!(
            p.kind(),
            "block"
                | "constructor_body"
                | "switch_block_statement_group"
                | "labeled_statement"
                | "if_statement"
                | "while_statement"
                | "for_statement"
                | "enhanced_for_statement"
                | "do_statement"
        )
    });
    let children = named_children(body);
    if !is_statement || children.iter().any(|c| c.kind() != "switch_block_statement_group") {
        return None;
    }
    let mut rules = Vec::new();
    let mut labels: Vec<Node> = Vec::new();
    for (index, group) in children.iter().enumerate() {
        let parts = named_children(group);
        labels.extend(parts.iter().filter(|c| c.kind() == "switch_label"));
        let mut statements: Vec<Node> =
            parts.iter().copied().filter(|c| c.kind() != "switch_label" && !is_comment(c)).collect();
        // `case 1: { ...; break; }`: the block's statements.
        let mut parts = parts;
        if let [block] = statements.as_slice()
            && block.kind() == "block"
            && last_statement(block).is_some_and(|s| s.kind() == "break_statement")
        {
            parts = named_children(block);
            statements = parts.iter().copied().filter(|c| !is_comment(c)).collect();
        }
        let Some(last) = statements.last() else {
            if index == children.len() - 1 {
                // Labels at the end without statements.
                rules.push(format!("{} -> {{}}", rule_labels(&labels, text)?));
            }
            continue;
        };
        let is_last_group = index == children.len() - 1;
        let kept = match last.kind() {
            "break_statement" if named_children(last).is_empty() => &statements[..statements.len() - 1],
            "return_statement" | "throw_statement" | "continue_statement" => &statements[..],
            _ if is_last_group => &statements[..],
            _ => return None,
        };
        // Comments must be among the kept statements, which are copied with what is between them.
        // Comments before a removed `break` are kept too.
        let end = match kept.last() {
            Some(_) if kept.len() < statements.len() => text[..last.start_byte()].trim_end().len(),
            Some(l) => l.end_byte(),
            None => 0,
        };
        let kept_range = kept.first().map_or(0..0, |f| f.start_byte()..end);
        let comments_outside = parts.iter().any(|c| is_comment(c) && !kept_range.contains(&c.start_byte()));
        if comments_outside || kept.iter().any(breaks_switch) || declares_shared(kept, group, &children, text) {
            return None;
        }
        let code = match kept {
            [] => "{}".to_string(),
            [single]
                if matches!(single.kind(), "expression_statement" | "throw_statement" | "block")
                    && kept_range.end == single.end_byte() =>
            {
                source(single, text).to_string()
            }
            _ => format!("{{\n{}\n}}", &text[kept_range]),
        };
        rules.push(format!("{} -> {code}", rule_labels(&labels, text)?));
        labels.clear();
    }
    Some(Edit::new(body.byte_range(), format!("{{\n{}\n}}", rules.join("\n"))))
}

/// The labels of a rule: `case A, B` or `default`.
fn rule_labels(labels: &[Node], text: &str) -> Option<String> {
    let mut cases = Vec::new();
    let mut patterns = 0;
    for label in labels {
        let code = source(label, text);
        match code.strip_prefix("case") {
            Some(rest) => {
                cases.push(rest.trim().to_string());
                patterns += usize::from(named_children(label).iter().any(|c| matches!(c.kind(), "pattern" | "guard")));
            }
            None if labels.len() == 1 => return Some("default".to_string()),
            // `default` with other labels.
            None => return None,
        }
    }
    (patterns <= usize::from(cases.len() == 1)).then(|| format!("case {}", cases.join(", ")))
}

/// Whether a `break` in the statements ends the switch (not a loop or switch inside them).
fn breaks_switch(statement: &Node) -> bool {
    if statement.kind() == "break_statement" {
        return named_children(statement).is_empty();
    }
    let opaque = &[
        "lambda_expression",
        "class_body",
        "for_statement",
        "enhanced_for_statement",
        "while_statement",
        "do_statement",
        "switch_expression",
    ];
    if opaque.contains(&statement.kind()) {
        return false;
    }
    let mut found = false;
    crate::lint::syntax::for_each_descendant(statement, opaque, |n| {
        found |= n.kind() == "break_statement" && named_children(n).is_empty();
    });
    found
}

/// Whether a variable declared by the statements of a group is named in another group: in a rule it would be out of
/// scope there.
fn declares_shared(statements: &[Node], group: &Node, groups: &[Node], text: &str) -> bool {
    let names: Vec<&str> = statements
        .iter()
        .filter(|s| s.kind() == "local_variable_declaration")
        .flat_map(|d| named_children(d))
        .filter(|d| d.kind() == "variable_declarator")
        .filter_map(|d| d.child_by_field_name("name"))
        .map(|n| source(&n, text))
        .collect();
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    names.iter().any(|name| {
        groups.iter().filter(|g| *g != group).any(|g| {
            let code = source(g, text);
            code.match_indices(name).any(|(i, _)| {
                !code[..i].chars().next_back().is_some_and(is_word)
                    && !code[i + name.len()..].chars().next().is_some_and(is_word)
            })
        })
    })
}

/// The last statement of a group or block, ignoring comments; `None` if it has none. An empty statement (`;`, an
/// anonymous node) counts: it completes normally.
fn last_statement<'t>(list: &Node<'t>) -> Option<Node<'t>> {
    let mut cursor = list.walk();
    let statements =
        list.children(&mut cursor).filter(|c| (c.is_named() || c.kind() == ";") && c.kind() != "switch_label");
    statements.filter(|c| !is_comment(c)).last()
}
