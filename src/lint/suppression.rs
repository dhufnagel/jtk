//! Suppressed violations: by a comment on the violation's line, or by `@SuppressWarnings` on an enclosing
//! declaration. Existing Checkstyle and PMD suppressions apply to the equivalent rules, so that switching to jtk does
//! not bring back violations a project has already accepted.
//!
//! - `// jtk:ignore` (all rules) or `// jtk:ignore[S001, B003]` on the line
//! - `// NOPMD` on the line (rules with a PMD equivalent)
//! - `@SuppressWarnings` with `"jtk:S001"`, `"checkstyle:needbraces"` or `"NeedBraces"` (case-insensitive, as in
//!   Checkstyle's `SuppressWarningsHolder`), `"PMD.ControlStatementBraces"` or `"PMD"` (all rules with a PMD
//!   equivalent)

use std::ops::Range;

use tree_sitter::Node;

use super::{Diagnostic, Rule, for_each_node, source};

const IGNORE_COMMENT: &str = "jtk:ignore";
const NOPMD_COMMENT: &str = "NOPMD";

pub struct Suppressions {
    line_starts: Vec<usize>,
    /// Lines (0-based) with a suppression comment, and the codes it names (empty for all rules).
    lines: Vec<(usize, LineSuppression)>,
    /// Declarations annotated with `@SuppressWarnings`, and the annotation's strings.
    declarations: Vec<(Range<usize>, Vec<String>)>,
}

enum LineSuppression {
    Codes(Vec<String>),
    All,
    Pmd,
}

impl Suppressions {
    pub fn find(root: &Node, text: &str) -> Suppressions {
        let line_starts = crate::newlines::line_offsets(text);
        let mut suppressions = Suppressions { line_starts, lines: Vec::new(), declarations: Vec::new() };
        // Most files suppress nothing; finding that in the text is much cheaper than walking the tree.
        if !(text.contains(IGNORE_COMMENT) || text.contains(NOPMD_COMMENT) || text.contains("SuppressWarnings")) {
            return suppressions;
        }
        for_each_node(root, |node| match node.kind() {
            "line_comment" | "block_comment" => suppressions.add_comment(node, text),
            "annotation" => suppressions.add_annotation(node, text),
            _ => {}
        });
        suppressions
    }

    pub fn suppresses(&self, diagnostic: &Diagnostic) -> bool {
        let line = self.line(diagnostic.offset);
        let by_comment = self.lines.iter().any(|(l, s)| *l == line && line_suppresses(s, diagnostic.rule));
        let by_annotation = self.declarations.iter().any(|(range, strings)| {
            range.contains(&diagnostic.offset) && strings.iter().any(|s| warning_suppresses(s, diagnostic.rule))
        });
        by_comment || by_annotation
    }

    fn line(&self, offset: usize) -> usize {
        self.line_starts.partition_point(|&start| start <= offset) - 1
    }

    fn add_comment(&mut self, comment: &Node, text: &str) {
        let content = source(comment, text);
        let suppression = if let Some(index) = content.find(IGNORE_COMMENT) {
            ignore_comment(&content[index + IGNORE_COMMENT.len()..])
        } else if content.contains(NOPMD_COMMENT) {
            LineSuppression::Pmd
        } else {
            return;
        };
        self.lines.push((self.line(comment.start_byte()), suppression));
    }

    fn add_annotation(&mut self, annotation: &Node, text: &str) {
        let Some(name) = annotation.child_by_field_name("name") else { return };
        if !source(&name, text).ends_with("SuppressWarnings") {
            return;
        }
        let modifiers = annotation.parent().filter(|p| p.kind() == "modifiers");
        let Some(declaration) = modifiers.and_then(|m| m.parent()) else { return };
        let mut strings = Vec::new();
        for_each_node(annotation, |node| {
            if node.kind() == "string_literal" {
                strings.push(source(node, text).trim_matches('"').to_string());
            }
        });
        self.declarations.push((declaration.byte_range(), strings));
    }
}

/// Parses what follows `jtk:ignore`: `[S001, B003]` names codes, anything else suppresses all rules.
fn ignore_comment(rest: &str) -> LineSuppression {
    let codes = rest.strip_prefix('[').and_then(|r| r.split_once(']')).map(|(codes, _)| codes);
    match codes {
        Some(codes) => LineSuppression::Codes(codes.split(',').map(|c| c.trim().to_string()).collect()),
        None => LineSuppression::All,
    }
}

fn line_suppresses(suppression: &LineSuppression, rule: Rule) -> bool {
    match suppression {
        LineSuppression::Codes(codes) => codes.iter().any(|c| c == rule.code()),
        LineSuppression::All => true,
        LineSuppression::Pmd => !rule.pmd_names().is_empty(),
    }
}

fn warning_suppresses(warning: &str, rule: Rule) -> bool {
    if let Some(code) = warning.strip_prefix("jtk:") {
        return code == rule.code();
    }
    // The token javac and IDEs use for unused code.
    if warning == "unused" {
        return matches!(rule, Rule::UnusedLocalVariable | Rule::UnusedPrivateMember | Rule::UnusedParameter);
    }
    if warning == "PMD" {
        return !rule.pmd_names().is_empty();
    }
    if let Some(name) = warning.strip_prefix("PMD.") {
        return rule.pmd_names().contains(&name);
    }
    let checkstyle_name = warning.strip_prefix("checkstyle:").unwrap_or(warning);
    rule.checkstyle_names().iter().any(|n| n.eq_ignore_ascii_case(checkstyle_name))
}
