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

use jtk_parser::ast::{EnumConstant, K, LitKind, Mods, Tree, VarDecl};

use super::tree::{Tokens, for_each_descendant};
use super::{Diagnostic, Rule};

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
    pub fn find(unit: &Tree, text: &str, tokens: &Tokens) -> Suppressions {
        let line_starts = crate::newlines::line_offsets(text);
        let mut suppressions = Suppressions { line_starts, lines: Vec::new(), declarations: Vec::new() };
        // Most files suppress nothing; finding that in the text is much cheaper than walking the tree.
        if text.contains(IGNORE_COMMENT) || text.contains(NOPMD_COMMENT) {
            for comment in tokens.comments() {
                suppressions.add_comment(comment, text);
            }
        }
        if text.contains("SuppressWarnings") {
            suppressions.add_declaration(unit, text);
            for_each_descendant(unit, &mut |tree, _| {
                suppressions.add_declaration(tree, text);
                true
            });
        }
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

    fn add_comment(&mut self, comment: Range<usize>, text: &str) {
        let content = &text[comment.clone()];
        let suppression = if let Some(index) = content.find(IGNORE_COMMENT) {
            ignore_comment(&content[index + IGNORE_COMMENT.len()..])
        } else if content.contains(NOPMD_COMMENT) {
            LineSuppression::Pmd
        } else {
            return;
        };
        self.lines.push((self.line(comment.start), suppression));
    }

    /// Adds the `@SuppressWarnings` of the declarations in `tree` that are not trees of their own: parameters,
    /// catch parameters, enum constants and the like, or of `tree` itself.
    fn add_declaration(&mut self, tree: &Tree, text: &str) {
        match &tree.kind {
            K::Class(class) => {
                self.add_modifiers(&class.mods, tree.start..tree.end, text);
                class.record_components.iter().for_each(|v| self.add_var(v, text));
                let constants = &class.enum_constants;
                for (i, constant) in constants.iter().enumerate() {
                    let end = constants
                        .get(i + 1)
                        .map_or_else(|| enum_constants_end(class.members.first(), tree), |c| c.mods.start);
                    self.add_enum_constant(constant, end, text);
                }
            }
            K::Method(method) => {
                self.add_modifiers(&method.mods, tree.start..tree.end, text);
                method.params.iter().for_each(|v| self.add_var(v, text));
            }
            K::Vars(vars) => {
                if let Some(first) = vars.first() {
                    self.add_modifiers(&first.mods, tree.start..tree.end, text);
                }
            }
            K::Var(v) | K::BindingPattern { var: v } | K::ForEach { var: v, .. } => self.add_var(v, text),
            K::Lambda { params, .. } => params.iter().for_each(|v| self.add_var(v, text)),
            K::Try { catches, .. } => catches.iter().for_each(|c| self.add_var(&c.param, text)),
            _ => {}
        }
    }

    fn add_var(&mut self, var: &VarDecl, text: &str) {
        self.add_modifiers(&var.mods, var.start..var.end, text);
    }

    fn add_enum_constant(&mut self, constant: &EnumConstant, end: usize, text: &str) {
        self.add_modifiers(&constant.mods, constant.mods.start..end, text);
    }

    fn add_modifiers(&mut self, mods: &Mods, declaration: Range<usize>, text: &str) {
        for annotation in &mods.annotations {
            let K::Annotation { ty, .. } = &annotation.kind else { continue };
            if !text[ty.start..ty.end].ends_with("SuppressWarnings") {
                continue;
            }
            let mut strings = Vec::new();
            for_each_descendant(annotation, &mut |tree, _| {
                if matches!(tree.kind, K::Literal(LitKind::String)) {
                    strings.push(text[tree.start..tree.end].trim_matches('"').to_string());
                }
                true
            });
            self.declarations.push((declaration.clone(), strings));
        }
    }
}

/// Where the last enum constant ends: before the first member, or at the end of the enum.
fn enum_constants_end(first_member: Option<&Tree>, class: &Tree) -> usize {
    first_member.map_or(class.end, |m| m.start)
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
