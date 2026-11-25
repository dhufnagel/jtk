//! M009: string literals concatenated over several lines that form lines of text (Eclipse save action "Use text
//! blocks"), Java 15: a text block is easier to read.
//!
//! Reported for a concatenation of only string literals (at least two) on more than one line, where every literal
//! but the last ends in `\n`. Eclipse converts any concatenation of three or more literals, whatever their line
//! ends; jtk keeps to text that is made of lines, which reads better as a text block. Not reported with comments
//! inside.
//!
//! Safe fix: the text block, indented like the second literal's line. Escapes stay as they are, except that `\n`
//! becomes a line break and `\"` a quote; quotes that would end the block are escaped, a space ending a line becomes
//! `\s` (text blocks strip trailing spaces), and text without a final line break ends with a `\` line continuation.
//! The value stays the same because string literals contain no line terminators other than `\n` escapes, and
//! formatters (jtk's, Eclipse's) keep the text block's lines.

use crate::lint::tree::Context;
use crate::lint::{Diagnostic, Edit, Rule};
use jtk_parser::ast::{K, LitKind, Tree};

pub fn check(binary: &Tree, context: &Context, diagnostics: &mut Vec<Diagnostic>) {
    let is_concatenation = |t: &Tree| matches!(&t.kind, K::Binary { op, .. } if op == "+");
    // Only the whole concatenation (`"a\n" + "b\n" + "c"` nests to the left).
    if !is_concatenation(binary) || context.parent().is_some_and(is_concatenation) {
        return;
    }
    let text = context.text;
    let Some(literals) = literal_operands(binary, context) else { return };
    let contents: Vec<&str> = literals.iter().map(|l| literal_content(l, text)).collect();
    let lines_end = contents[..contents.len() - 1].iter().all(|c| ends_with_newline_escape(c));
    let spans_lines = context.source(binary).contains('\n');
    if literals.len() < 2 || !lines_end || !spans_lines {
        return;
    }
    let message = "String concatenation can be a text block";
    let indentation = line_indentation(literals[1].start, text);
    let block = text_block(&contents.concat(), indentation);
    let fix = Edit::new(binary.start..binary.end, block);
    diagnostics.push(Diagnostic::new(Rule::TextBlock, binary.start, message).with_fix(vec![fix]));
}

/// The operands of a `+` concatenation if all are string literals (not text blocks) and no comment is between
/// them, in order.
fn literal_operands<'t>(binary: &'t Tree, context: &Context) -> Option<Vec<&'t Tree>> {
    if context.tokens().has_comment(binary.start..binary.end) {
        return None;
    }
    let mut operands = Vec::new();
    let mut pending = vec![binary];
    while let Some(tree) = pending.pop() {
        match &tree.kind {
            K::Binary { op, lhs, rhs } if op == "+" => {
                pending.push(rhs);
                pending.push(lhs);
            }
            K::Literal(LitKind::String) if !context.source(tree).starts_with("\"\"\"") => operands.push(tree),
            _ => return None,
        }
    }
    Some(operands)
}

/// The text between the quotes of a string literal, with its escapes.
fn literal_content<'a>(literal: &Tree, text: &'a str) -> &'a str {
    let code = &text[literal.start..literal.end];
    &code[1..code.len() - 1]
}

/// Whether the literal's content ends in a `\n` escape (not an escaped backslash followed by `n`).
fn ends_with_newline_escape(content: &str) -> bool {
    let Some(before) = content.strip_suffix("\\n") else { return false };
    let backslashes = before.len() - before.trim_end_matches('\\').len();
    backslashes % 2 == 0
}

fn line_indentation(offset: usize, text: &str) -> &str {
    let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
    let line = &text[line_start..];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

/// The text block with the content of the literals (with their escapes), each line indented.
fn text_block(content: &str, indentation: &str) -> String {
    // Split into lines at `\n` escapes, unescaping `\"`; other escapes are kept.
    let mut lines = vec![String::new()];
    let mut chars = content.chars();
    while let Some(c) = chars.next() {
        let line = lines.last_mut().expect("there is a line");
        if c != '\\' {
            line.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => lines.push(String::new()),
            Some('"') => line.push('"'),
            Some(escaped) => {
                line.push('\\');
                line.push(escaped);
            }
            None => line.push('\\'),
        }
    }
    let ends_with_newline = lines.last().is_some_and(String::is_empty);
    if ends_with_newline {
        lines.pop();
    }
    let mut block = String::from("\"\"\"\n");
    let count = lines.len();
    for (i, line) in lines.iter().enumerate() {
        let mut line = escape_quotes(line, !ends_with_newline && i == count - 1);
        if line.ends_with(' ') {
            line.pop();
            line.push_str("\\s");
        } else if line.ends_with('\t') {
            line.pop();
            line.push_str("\\t");
        }
        if !line.is_empty() {
            block.push_str(indentation);
            block.push_str(&line);
        }
        if i == count - 1 && !ends_with_newline {
            block.push('\\');
        }
        block.push('\n');
    }
    block.push_str(indentation);
    block.push_str("\"\"\"");
    block
}

/// Escapes every third quote in a row, so that no `"""` ends the block early; and a final quote before the closing
/// delimiter of the last line if `last` (the continuation `\` follows it, but escaping keeps it clear).
fn escape_quotes(line: &str, last: bool) -> String {
    let mut escaped = String::new();
    let mut run = 0;
    let mut chars = line.chars().peekable();
    let mut after_backslash = false;
    while let Some(c) = chars.next() {
        if after_backslash {
            escaped.push(c);
            after_backslash = false;
            run = 0;
            continue;
        }
        match c {
            '\\' => {
                escaped.push(c);
                after_backslash = true;
                run = 0;
            }
            '"' => {
                run += 1;
                if run == 3 || (last && chars.peek().is_none()) {
                    escaped.push_str("\\\"");
                    run = 0;
                } else {
                    escaped.push('"');
                }
            }
            _ => {
                escaped.push(c);
                run = 0;
            }
        }
    }
    escaped
}
