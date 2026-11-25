//! B150: a date pattern with the week year `Y` but no week `w`, or with the day of year `D` and a month `M`: `yyyy`
//! and `dd` are meant, and the dates are wrong around new year or after January (Error Prone MisusedWeekYear and
//! MisusedDayOfYear, IntelliJ SuspiciousDateFormat).
//!
//! Checked are string literals passed to `new SimpleDateFormat(pattern, ...)` (`java.text` or ICU) and
//! `DateTimeFormatter.ofPattern(pattern, ...)` (`java.time.format`), with the type resolved through the imports;
//! unlike Error Prone, not `applyPattern` (the receiver's type is unknown) nor constants. Quoted text in the pattern
//! is skipped. The fix replaces `Y` by `y` and `D` by `d` outside quotes, for literals without escapes. It is safe in
//! the catalogue's sense: it changes the behaviour, but the week year without weeks and the day of year with months
//! are not what is meant.

use super::shared_misc::{names_type, string_value};
use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};

const SIMPLE_DATE_FORMATS: &[&str] = &["java.text.SimpleDateFormat", "com.ibm.icu.text.SimpleDateFormat"];

pub fn check(tree: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let pattern = match &tree.kind {
        K::NewClass { ident, args, .. } if SIMPLE_DATE_FORMATS.iter().any(|t| names_type(ident, t, context)) => {
            args.first()
        }
        K::MethodInvocation { select, args, .. } => match &select.kind {
            K::MemberSelect { expr, name }
                if name == "ofPattern" && names_type(expr, "java.time.format.DateTimeFormatter", context) =>
            {
                args.first()
            }
            _ => None,
        },
        _ => None,
    };
    let Some(pattern) = pattern else { return };
    let Some(value) = string_value(pattern, context.text) else { return };
    let letters: String = unquoted(&value).map(|(_, c)| c).collect();
    let week_year = letters.contains('Y') && !letters.contains('w');
    let day_of_year = letters.contains('D') && letters.contains('M');
    let message = match (week_year, day_of_year) {
        (false, false) => return,
        (true, false) => "Week year \"Y\" in a date pattern without week \"w\"",
        (false, true) => "Day of year \"D\" in a date pattern with month \"M\"",
        (true, true) => {
            "Week year \"Y\" and day of year \"D\" in a date pattern with month \"M\" but without week \"w\""
        }
    };
    let diagnostic = Diagnostic::new(Rule::SuspiciousDatePattern, pattern.start, message);
    let code = context.source(pattern);
    if code.contains('\\') {
        diagnostics.push(diagnostic);
        return;
    }
    // Without escapes, the literal's content is the value.
    let mut fixed: Vec<char> = value.chars().collect();
    for (i, c) in unquoted(&value) {
        if week_year && c == 'Y' || day_of_year && c == 'D' {
            fixed[i] = c.to_ascii_lowercase();
        }
    }
    let replacement = format!("\"{}\"", fixed.into_iter().collect::<String>());
    diagnostics.push(diagnostic.with_fix(vec![Edit::new(pattern.start..pattern.end, replacement)]));
}

/// The characters of the pattern outside quotes (`'text'`, where `''` is a quote), with their index.
fn unquoted(pattern: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    let mut quoted = false;
    pattern.chars().enumerate().filter(move |&(_, c)| {
        if c == '\'' {
            quoted = !quoted;
        }
        !quoted && c != '\''
    })
}
