//! Applying automatic fixes.

use super::{Diagnostic, Edit, Selection, Settings, lint_file};
use crate::formatter::FormatError;

/// Fixing one violation can uncover or move others (nested statements), so fixes are applied in rounds.
const MAX_ROUNDS: usize = 10;

/// Applies the safe fixes of the selected rules' violations until none is left, and returns the fixed text. Fixes
/// only need to produce valid code: formatting the result restores the layout.
pub fn fix(text: &str, selection: &Selection) -> Result<String, FormatError> {
    fix_with(text, &Settings::from(selection.clone()), false)
}

/// Like [`fix`] with the given rule parameters, also applying the unsafe fixes if `unsafe_fixes` is set: those that may change the behaviour in
/// edge cases (see [`Diagnostic::unsafe_fix`]).
pub fn fix_with(text: &str, settings: &Settings, unsafe_fixes: bool) -> Result<String, FormatError> {
    let mut text = text.to_string();
    for _ in 0..MAX_ROUNDS {
        let mut diagnostics = lint_file(&text, None, settings)?;
        diagnostics.retain(|d| unsafe_fixes || !d.unsafe_fix);
        let edits = non_overlapping_edits(&diagnostics);
        if edits.is_empty() {
            break;
        }
        text = apply(&text, &edits);
    }
    Ok(text)
}

/// Applies the fix of one violation.
pub fn apply_fix(text: &str, diagnostic: &Diagnostic) -> String {
    let mut edits: Vec<&Edit> = diagnostic.fix.iter().collect();
    edits.sort_by_key(|e| e.range.start);
    apply(text, &edits)
}

/// The edits of all fixes that do not overlap an earlier fix; the others are left to the next round.
fn non_overlapping_edits(diagnostics: &[Diagnostic]) -> Vec<&Edit> {
    let mut accepted: Vec<&Edit> = Vec::new();
    for diagnostic in diagnostics {
        let overlaps =
            |edit: &Edit| accepted.iter().any(|a| edit.range.start < a.range.end && a.range.start < edit.range.end);
        if !diagnostic.fix.iter().any(overlaps) {
            accepted.extend(&diagnostic.fix);
        }
    }
    accepted.sort_by_key(|e| e.range.start);
    accepted
}

/// Applies edits sorted by position that do not overlap. A replacement that would join two words (`case(1)` to
/// `case1`) is separated from them by a space.
fn apply(text: &str, edits: &[&Edit]) -> String {
    let mut result = String::with_capacity(text.len());
    let mut position = 0;
    for edit in edits {
        result.push_str(&text[position..edit.range.start]);
        let replacement = &edit.replacement;
        if ends_with_word(&result) && starts_with_word(replacement) {
            result.push(' ');
        }
        result.push_str(replacement);
        if ends_with_word(replacement) && starts_with_word(&text[edit.range.end..]) {
            result.push(' ');
        }
        position = edit.range.end;
    }
    result.push_str(&text[position..]);
    result
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

fn starts_with_word(text: &str) -> bool {
    text.chars().next().is_some_and(is_word_char)
}

fn ends_with_word(text: &str) -> bool {
    text.chars().next_back().is_some_and(is_word_char)
}
