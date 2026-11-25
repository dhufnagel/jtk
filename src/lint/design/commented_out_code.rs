//! A101: a comment that contains Java code (IntelliJ CommentedOutCode, Sonar S125): commented-out code is dead
//! weight that nobody maintains; version control keeps the old code.
//!
//! Consecutive `//` comments on lines of their own form one block; a `/* */` comment on lines of its own is a block
//! (Javadoc `/** */` and Markdown doc comments `///` never are). A block is reported if its text parses as Java
//! statements (in a method body), class members, or a compilation unit (imports, package, types) and also looks like
//! code: it ends with `;`, `{` or `}` and has a keyword or one of `(`, `=`, `@`, `{`. Prose that the parser accepts
//! is left out: expression statements other than assignments, increments, calls and `new` (`see this;`), and
//! labelled statements (`Example: run();`). Comments with `TODO`, `FIXME`, `NOSONAR`, suppression or formatter
//! markers, `language=` or URLs, comments after code on the same line, and blocks right after a comment line ending
//! with `:` (an example: `For example:`) are never reported.
//!
//! Unsafe fix: removes the block with its whole lines (and a blank line after it if one is also before it). It does
//! not change the behaviour, but deletes what the author may have kept on purpose.

use crate::lint::tree::{Context, for_each_descendant};
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, Tree};
use jtk_parser::lexer::{RawKind, lex};
use jtk_parser::parser::parse;
use std::ops::Range;

/// Markers of comments that are not commented-out code even if they parse.
const MARKERS: &[&str] =
    &["TODO", "FIXME", "NOSONAR", "NOPMD", "noinspection", "CHECKSTYLE", "@formatter", "language=", "$NON-NLS", "://"];

const KEYWORDS: &[&str] = &[
    "abstract",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "continue",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "true",
    "try",
    "void",
    "while",
];

pub fn check(_: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let text = context.text;
    for (range, code) in blocks(context) {
        if !is_code(&code) || is_example(range.start, text) {
            continue;
        }
        let diagnostic = Diagnostic::new(Rule::CommentedOutCode, range.start, "commented-out code");
        diagnostics.push(diagnostic.with_unsafe_fix(vec![Edit::new(removal_range(range, text), "")]));
    }
}

/// The comment blocks on lines of their own, with their text without the comment delimiters.
fn blocks(context: &Context) -> Vec<(Range<usize>, String)> {
    let text = context.text;
    let mut blocks: Vec<(Range<usize>, String)> = Vec::new();
    let mut previous_line_comment: Option<usize> = None;
    for comment in context.tokens().comments() {
        let source = &text[comment.clone()];
        let line_start = text[..comment.start].rfind('\n').map_or(0, |i| i + 1);
        let line_end = text[comment.end..].find('\n').map_or(text.len(), |i| comment.end + i);
        let alone = text[line_start..comment.start].trim().is_empty() && text[comment.end..line_end].trim().is_empty();
        if !alone {
            previous_line_comment = None;
            continue;
        }
        if let Some(line) = source.strip_prefix("//") {
            if line.starts_with('/') {
                previous_line_comment = None;
                continue;
            }
            let continues = previous_line_comment.is_some_and(|end| {
                let between = &text[end..comment.start];
                between.trim().is_empty() && between.matches('\n').count() == 1
            });
            match blocks.last_mut() {
                Some((range, code)) if continues => {
                    range.end = comment.end;
                    code.push('\n');
                    code.push_str(line);
                }
                _ => blocks.push((comment.clone(), line.to_string())),
            }
            previous_line_comment = Some(comment.end);
        } else {
            previous_line_comment = None;
            if source.starts_with("/**") {
                continue;
            }
            let Some(inner) = source.strip_prefix("/*").and_then(|s| s.strip_suffix("*/")) else { continue };
            let lines: Vec<&str> = inner.lines().map(|l| l.trim_start()).collect();
            let starred = lines.iter().skip(1).all(|l| l.is_empty() || l.starts_with('*'));
            let code = lines.iter().map(|l| if starred { l.strip_prefix('*').unwrap_or(l) } else { l });
            blocks.push((comment.clone(), code.collect::<Vec<_>>().join("\n")));
        }
    }
    blocks
}

/// Whether the comment right before the block (on the lines above, ignoring a line with only `*/`) ends with `:`,
/// introducing the block as an example.
fn is_example(start: usize, text: &str) -> bool {
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let mut lines = text[..line_start].lines().rev().map(str::trim);
    let Some(mut previous) = lines.next() else { return false };
    if previous == "*/" {
        let Some(line) = lines.next() else { return false };
        previous = line;
    }
    let is_comment = previous.starts_with("//") || previous.starts_with('*') || previous.starts_with("/*");
    is_comment && previous.trim_end_matches("*/").trim_end().ends_with(':')
}

/// Whether the text of a comment is Java code.
fn is_code(code: &str) -> bool {
    let trimmed = code.trim();
    if !trimmed.ends_with([';', '{', '}']) || MARKERS.iter().any(|m| code.contains(m)) || !has_code_token(code) {
        return false;
    }
    let statements = format!("class X {{ void m() {{\n{code}\n}} }}");
    let members = format!("class X {{\n{code}\n}}");
    [statements, members, code.to_string()].iter().any(|source| parse(source).is_ok_and(|unit| is_plausible(&unit)))
}

/// Whether the code has a keyword or one of `(`, `=`, `@`, `{`, so that it is not just words.
fn has_code_token(code: &str) -> bool {
    let Ok((tokens, _)) = lex(code, false) else { return false };
    tokens.iter().filter(|(kind, _, _)| *kind == RawKind::Token).any(|&(_, start, end)| {
        let token = &code[start..end];
        let is_literal = token.starts_with(['"', '\'']);
        KEYWORDS.contains(&token) || matches!(token, "(" | "@" | "{") || token.contains('=') && !is_literal
    })
}

/// Whether the parsed code has no expression statement javac would reject (`see this;`) and no labelled statement.
fn is_plausible(unit: &Tree) -> bool {
    let mut plausible = true;
    for_each_descendant(unit, &mut |tree, _| {
        plausible &= match &tree.kind {
            K::Labeled { .. } => false,
            K::ExprStmt(expression) => {
                matches!(&expression.kind, K::Assign { .. } | K::MethodInvocation { .. } | K::NewClass { .. })
                    || matches!(&expression.kind, K::Unary { op, .. } if op == "++" || op == "--")
            }
            _ => true,
        };
        plausible
    });
    plausible
}

/// The whole lines of the block, and a blank line after it if the line before it is blank too.
fn removal_range(range: Range<usize>, text: &str) -> Range<usize> {
    let start = text[..range.start].rfind('\n').map_or(0, |i| i + 1);
    let end = text[range.end..].find('\n').map_or(text.len(), |i| range.end + i + 1);
    let before_blank = start >= 1 && {
        let previous_start = text[..start - 1].rfind('\n').map_or(0, |i| i + 1);
        text[previous_start..start].trim().is_empty()
    };
    let next_end = text[end..].find('\n').map(|i| end + i + 1);
    match next_end {
        Some(next_end) if before_blank && text[end..next_end].trim().is_empty() => start..next_end,
        _ => start..end,
    }
}
