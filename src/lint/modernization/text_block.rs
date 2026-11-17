//! M009: string literals concatenated over several lines that form lines of text (Eclipse save action "Use text
//! blocks"), Java 15: a text block is easier to read.
//!
//! Reported for a concatenation of only string literals (at least two) on more than one line, where every literal
//! but the last ends in `\n`. Eclipse converts any concatenation of three or more literals, whatever their line
//! ends; jtk keeps to text that is made of lines, which reads better as a text block. Not reported with comments
//! inside.
//!
//! Unsafe fix: the text block, indented like the second literal's line. Escapes stay as they are, except that `\n`
//! becomes a line break and `\"` a quote; quotes that would end the block are escaped, a space ending a line becomes
//! `\s` (text blocks strip trailing spaces), and text without a final line break ends with a `\` line continuation.
//! Unsafe because the value only stays the same where the source has no line terminator other than `\n` escapes
//! (always the case for literals) and the formatter keeps the text block's lines, which jtk's does.

use std::cell::Cell;

use tree_sitter::Node;

use crate::lint::syntax::{named_children, operator, source};
use crate::lint::{Diagnostic, Edit, Rule};

pub fn check(binary: &Node, text: &str, diagnostics: &mut Vec<Diagnostic>) {
    if binary.kind() == "program" {
        // A new file: forget the last one's concatenation.
        LAST_WHOLE.set((0, 0, 0));
        return;
    }
    if operator(binary, text) != "+" || is_inside_last_whole(binary, text) {
        return;
    }
    // Only the whole concatenation (`"a\n" + "b\n" + "c"` nests to the left).
    if binary.parent().is_some_and(|p| p.kind() == "binary_expression" && operator(&p, text) == "+") {
        return;
    }
    LAST_WHOLE.set((text.as_ptr() as usize, binary.start_byte(), binary.end_byte()));
    let Some(literals) = literal_operands(binary, text) else { return };
    let contents: Vec<&str> = literals.iter().map(|l| literal_content(l, text)).collect();
    let lines_end = contents[..contents.len() - 1].iter().all(|c| ends_with_newline_escape(c));
    let spans_lines = text[binary.byte_range()].contains('\n');
    if literals.len() < 2 || !lines_end || !spans_lines {
        return;
    }
    let message = "String concatenation can be a text block";
    let indentation = line_indentation(literals[1].start_byte(), text);
    let block = text_block(&contents.concat(), indentation);
    let fix = Edit::new(binary.byte_range(), block);
    diagnostics.push(Diagnostic::new(Rule::TextBlock, binary.start_byte(), message).with_unsafe_fix(vec![fix]));
}

thread_local! {
    /// The text and byte range of the last whole concatenation checked in the current file.
    static LAST_WHOLE: Cell<(usize, usize, usize)> = const { Cell::new((0, 0, 0)) };
}

/// Whether the `+` expression is on the left side of the last whole concatenation checked, which the walk visits
/// first: then its parent is a `+` too. This avoids looking up the parent, which takes time proportional to the depth
/// of the node, so that long concatenations (thousands of operands) would take quadratic time.
fn is_inside_last_whole(binary: &Node, text: &str) -> bool {
    let (whole_text, start, end) = LAST_WHOLE.get();
    whole_text == text.as_ptr() as usize && binary.start_byte() == start && binary.end_byte() < end
}

/// The operands of a `+` concatenation if all are string literals (not text blocks), in order.
fn literal_operands<'t>(binary: &Node<'t>, text: &str) -> Option<Vec<Node<'t>>> {
    let mut operands = Vec::new();
    let mut pending = vec![*binary];
    while let Some(node) = pending.pop() {
        match node.kind() {
            "binary_expression" if operator(&node, text) == "+" => {
                // Comments between the operands are children of the expression.
                let children = named_children(&node);
                if children.len() != 2 {
                    return None;
                }
                pending.push(children[1]);
                pending.push(children[0]);
            }
            "string_literal" if !source(&node, text).starts_with("\"\"\"") => operands.push(node),
            _ => return None,
        }
    }
    Some(operands)
}

/// The text between the quotes of a string literal, with its escapes.
fn literal_content<'a>(literal: &Node, text: &'a str) -> &'a str {
    let code = source(literal, text);
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
