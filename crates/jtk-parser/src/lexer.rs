// Derived from palantir-java-format's JavacTokens.java
// Copyright 2015-2019 Google Inc.; Copyright 2019-2025 Palantir Technologies Inc.
// Modified by the jtk authors: ported to Rust. Licensed under the Apache License, Version 2.0.

//! A Java lexer that splits source text into whitespace, comment and token ranges.
//!
//! Operators are lexed like javac (maximal munch); string, character and text block literals are single tokens.

/// Where and why lexing failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LexError {
    /// Byte offset of the start of the unterminated construct.
    pub offset: usize,
    pub message: &'static str,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for LexError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawKind {
    Whitespace,
    Comment,
    Token,
}

fn is_java_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c' | '\x0b' | '\x1c'..='\x1f')
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

fn is_ident_part(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// A raw tok: its kind and byte range.
pub type RawTok = (RawKind, usize, usize);

/// Lexes the input. With `stop_at_type`, lexing stops before the first `class`, `interface` or `enum` keyword, whose
/// position is returned (as with javac's stop tokens in `ImportOrderer`).
pub fn lex(text: &str, stop_at_type: bool) -> Result<(Vec<RawTok>, Option<usize>), LexError> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let n = bytes.len();
    let char_at = |i: usize| text[i..].chars().next().unwrap();
    while i < n {
        let c = char_at(i);
        let start = i;
        if is_java_whitespace(c) {
            while i < n && is_java_whitespace(char_at(i)) {
                i += char_at(i).len_utf8();
            }
            out.push((RawKind::Whitespace, start, i));
        } else if bytes[i] == b'/' && i + 1 < n && bytes[i + 1] == b'/' {
            while i < n && bytes[i] != b'\n' && bytes[i] != b'\r' {
                i += 1;
            }
            out.push((RawKind::Comment, start, i));
        } else if bytes[i] == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
            i += 2;
            loop {
                if i + 1 >= n {
                    return Err(LexError { offset: start, message: "unterminated comment" });
                }
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            out.push((RawKind::Comment, start, i));
        } else if text[i..].starts_with("\"\"\"") {
            i += 3;
            loop {
                if i >= n {
                    return Err(LexError { offset: start, message: "unterminated text block" });
                }
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i..].starts_with(b"\"\"\"") {
                    i += 3;
                    break;
                }
                i += 1;
            }
            out.push((RawKind::Token, start, i));
        } else if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            i += 1;
            loop {
                if i >= n || bytes[i] == b'\n' {
                    return Err(LexError { offset: start, message: "unterminated literal" });
                }
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push((RawKind::Token, start, i));
        } else if c.is_ascii_digit() || (c == '.' && i + 1 < n && bytes[i + 1].is_ascii_digit()) {
            i = lex_number(bytes, i);
            out.push((RawKind::Token, start, i));
        } else if is_ident_start(c) {
            while i < n && is_ident_part(char_at(i)) {
                i += char_at(i).len_utf8();
            }
            if stop_at_type && matches!(&text[start..i], "class" | "interface" | "enum") {
                return Ok((out, Some(start)));
            }
            out.push((RawKind::Token, start, i));
        } else {
            // Operators are lexed like javac (maximal munch) and split into single-character toks later.
            i += operator_len(&text[i..]).unwrap_or(c.len_utf8());
            out.push((RawKind::Token, start, i));
        }
    }
    Ok((out, None))
}

/// javac's operator tokens (plus `...`), longest first.
const OPERATORS: &[&str] = &[
    ">>>=", "<<=", ">>=", ">>>", "...", "->", "::", "==", "<=", ">=", "!=", "&&", "||", "++", "--", "<<", ">>", "+=",
    "-=", "*=", "/=", "&=", "|=", "^=", "%=",
];

pub fn operator_len(s: &str) -> Option<usize> {
    OPERATORS.iter().find(|op| s.starts_with(**op)).map(|op| op.len())
}

fn lex_number(b: &[u8], mut i: usize) -> usize {
    let n = b.len();
    let is_hex = |c: u8| c.is_ascii_hexdigit() || c == b'_';
    let is_dec = |c: u8| c.is_ascii_digit() || c == b'_';
    if b[i] == b'0' && i + 1 < n && (b[i + 1] == b'x' || b[i + 1] == b'X') {
        i += 2;
        while i < n && is_hex(b[i]) {
            i += 1;
        }
        if i < n && b[i] == b'.' {
            i += 1;
            while i < n && is_hex(b[i]) {
                i += 1;
            }
        }
        if i < n && (b[i] == b'p' || b[i] == b'P') {
            i += 1;
            if i < n && (b[i] == b'+' || b[i] == b'-') {
                i += 1;
            }
            while i < n && is_dec(b[i]) {
                i += 1;
            }
        }
    } else if b[i] == b'0' && i + 1 < n && (b[i + 1] == b'b' || b[i + 1] == b'B') {
        i += 2;
        while i < n && (b[i] == b'0' || b[i] == b'1' || b[i] == b'_') {
            i += 1;
        }
    } else {
        while i < n && is_dec(b[i]) {
            i += 1;
        }
        if i < n && b[i] == b'.' && !(i + 1 < n && b[i + 1] == b'.') {
            i += 1;
            while i < n && is_dec(b[i]) {
                i += 1;
            }
        }
        if i < n && (b[i] == b'e' || b[i] == b'E') {
            i += 1;
            if i < n && (b[i] == b'+' || b[i] == b'-') {
                i += 1;
            }
            while i < n && is_dec(b[i]) {
                i += 1;
            }
        }
    }
    if i < n && matches!(b[i], b'l' | b'L' | b'f' | b'F' | b'd' | b'D') {
        i += 1;
    }
    i
}
