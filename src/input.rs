// Derived from palantir-java-format's JavaInput.java and JavacTokens.java
// Copyright 2015-2019 Google Inc.; Copyright 2019-2025 Palantir Technologies Inc.
// Modified by the jtk authors: ported to Rust. Licensed under the Apache License, Version 2.0.

//! The formatter input: a lexed Java compilation unit (port of `JavaInput.java` and `JavacTokens.java`).
//!
//! The input is split into [`Tok`]s (tokens, comments, newlines and runs of blanks). Multi-character operators are
//! split into single-character toks, as in the original. Every non-token tok is attached to a [`Token`].

use crate::newlines::{self, jlen};

/// A closed-open range of tok indices. Empty ranges are represented by `lo >= hi`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub lo: i32,
    pub hi: i32,
}

impl Range {
    pub const EMPTY: Range = Range { lo: -1, hi: -1 };

    pub fn singleton(k: i32) -> Range {
        Range { lo: k, hi: k + 1 }
    }

    pub fn is_empty(&self) -> bool {
        self.lo >= self.hi
    }

    pub fn union(self, other: Range) -> Range {
        if self.is_empty() {
            other
        } else if other.is_empty() {
            self
        } else {
            Range { lo: self.lo.min(other.lo), hi: self.hi.max(other.hi) }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Tok {
    /// Sequential index for tokens and comments, `-1` for whitespace.
    pub index: i32,
    pub text: String,
    /// Byte offset in the input.
    pub position: usize,
    /// 0-based column (in UTF-16 units).
    pub column: usize,
    pub is_token: bool,
}

impl Tok {
    pub fn length(&self) -> usize {
        jlen(&self.text)
    }
    pub fn is_newline(&self) -> bool {
        newlines::is_newline(&self.text)
    }
    pub fn is_slash_slash_comment(&self) -> bool {
        self.text.starts_with("//")
    }
    pub fn is_slash_star_comment(&self) -> bool {
        self.text.starts_with("/*")
    }
    pub fn is_javadoc_comment(&self) -> bool {
        self.text.starts_with("/**") && self.text.len() > 4
    }
    pub fn is_comment(&self) -> bool {
        self.is_slash_slash_comment() || self.is_slash_star_comment()
    }
}

/// A token tok together with the non-token toks attached to it.
#[derive(Debug, Clone)]
pub struct Token {
    pub tok: usize,
    pub before: Vec<usize>,
    pub after: Vec<usize>,
}

pub type LexError = crate::formatter::FormatError;

pub struct JavaInput {
    pub text: String,
    pub toks: Vec<Tok>,
    pub tokens: Vec<Token>,
    /// Number of numbered toks, excluding EOF.
    pub kn: i32,
    /// For each input line, the range of tok indices lying on it.
    pub ranges: Vec<Range>,
    pub line_count: usize,
    line_starts: Vec<usize>,
    /// Sorted `(lo, hi, token index)` triples: closed position ranges covered by each token.
    position_tokens: Vec<(usize, usize, usize)>,
    k_to_token: Vec<usize>,
    pub line_separator: &'static str,
}

enum RawKind {
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

/// Lex the input into raw `(kind, start, end)` toks.
/// A raw tok: its kind and byte range.
type RawTok = (RawKind, usize, usize);

/// Lexes the input. With `stop_at_type`, lexing stops before the first `class`, `interface` or `enum` keyword, whose
/// position is returned (as with javac's stop tokens in `ImportOrderer`).
fn lex(text: &str, stop_at_type: bool) -> Result<(Vec<RawTok>, Option<usize>), LexError> {
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
                    return Err(LexError::at_offset(text, start, "unterminated comment"));
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
                    return Err(LexError::at_offset(text, start, "unterminated text block"));
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
                    return Err(LexError::at_offset(text, start, "unterminated literal"));
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

fn operator_len(s: &str) -> Option<usize> {
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

fn update_column(column: usize, text: &str) -> usize {
    let last = newlines::last_line_start(text);
    if last > 0 { jlen(&text[last..]) } else { column + jlen(text) }
}

pub(crate) fn build_toks(text: &str) -> Result<Vec<Tok>, LexError> {
    build_toks_until(text, false)
}

/// Like [`build_toks`], but with `stop_at_type` the toks end before the first type declaration keyword, followed by
/// an empty EOF tok at its position.
pub(crate) fn build_toks_until(text: &str, stop_at_type: bool) -> Result<Vec<Tok>, LexError> {
    let (raw, stop) = lex(text, stop_at_type)?;
    let end = stop.unwrap_or(text.len());
    let mut toks = Vec::new();
    let mut kn = 0;
    let mut column = 0;
    for (kind, start, end) in raw {
        let original = &text[start..end];
        let (is_token, numbered, strings): (bool, bool, Vec<&str>) = match kind {
            RawKind::Whitespace => {
                let mut strings = Vec::new();
                for line in newlines::lines(original) {
                    if let Some(nl) = newlines::line_ending(line) {
                        let spaces = &line[..line.len() - nl.len()];
                        if !spaces.is_empty() {
                            strings.push(spaces);
                        }
                        strings.push(&line[line.len() - nl.len()..]);
                    } else if !line.is_empty() {
                        strings.push(line);
                    }
                }
                (false, false, strings)
            }
            RawKind::Comment => (false, true, vec![original]),
            // Multi-character operators become one tok per character. Like the original, the column still
            // advances by the whole operator's length per character; `argumentsAreTabular` depends on that.
            RawKind::Token if original.len() > 1 && operator_len(original) == Some(original.len()) => {
                (true, true, (0..original.len()).map(|i| &original[i..i + 1]).collect())
            }
            RawKind::Token => (true, true, vec![original]),
        };
        let mut pos = start;
        for s in strings {
            let index = if numbered {
                kn += 1;
                kn - 1
            } else {
                -1
            };
            toks.push(Tok { index, text: s.to_string(), position: pos, column, is_token });
            pos += s.len();
            column = update_column(column, original);
        }
    }
    toks.push(Tok { index: kn, text: String::new(), position: end, column, is_token: true });
    Ok(toks)
}

fn is_param_comment(tok: &Tok) -> bool {
    // Matches `/\*[A-Za-z0-9\s_\-]+=\s*\*/`
    let t = &tok.text;
    if !tok.is_slash_star_comment() || !t.ends_with("*/") || t.len() < 5 {
        return false;
    }
    let inner = &t[2..t.len() - 2];
    let inner = inner.trim_end_matches(|c: char| c.is_whitespace());
    let Some(body) = inner.strip_suffix('=') else {
        return false;
    };
    !body.is_empty() && body.chars().all(|c| c.is_ascii_alphanumeric() || c.is_whitespace() || c == '_' || c == '-')
}

fn build_tokens(toks: &[Tok]) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut k = 0;
    let kn = toks.len();
    let mut before: Vec<usize> = Vec::new();
    'outermost: while k < kn {
        while !toks[k].is_token {
            let tok = k;
            k += 1;
            before.push(tok);
            if is_param_comment(&toks[tok]) {
                while toks[k].is_newline() {
                    k += 1;
                }
            }
        }
        let tok = k;
        k += 1;
        let mut after = Vec::new();
        while k < kn && !toks[k].is_token {
            if toks[k].is_slash_star_comment() && matches!(toks[tok].text.as_str(), "(" | "<" | ".") {
                break;
            }
            if toks[k].is_javadoc_comment() && toks[tok].text == ";" {
                break;
            }
            if is_param_comment(&toks[k]) {
                tokens.push(Token { tok, before: std::mem::take(&mut before), after });
                before = vec![k];
                k += 1;
                while toks[k].is_newline() {
                    k += 1;
                }
                continue 'outermost;
            }
            let non_token_after = k;
            k += 1;
            after.push(non_token_after);
            if newlines::contains_breaks(&toks[non_token_after].text) {
                break;
            }
        }
        tokens.push(Token { tok, before: std::mem::take(&mut before), after });
    }
    tokens
}

impl JavaInput {
    pub fn new(text: &str) -> Result<JavaInput, LexError> {
        let toks = build_toks(text)?;
        let kn = toks.last().unwrap().index;
        let line_count = newlines::lines(text).len();
        // computeRanges
        let mut ranges: Vec<Range> = Vec::new();
        let mut line_i = 0;
        for tok in &toks {
            let line_i0 = line_i;
            line_i += newlines::count(&tok.text);
            let k = tok.index;
            if k >= 0 {
                for i in line_i0..=line_i {
                    while ranges.len() <= i {
                        ranges.push(Range::EMPTY);
                    }
                    let old = ranges[i];
                    ranges[i] = Range { lo: if old.is_empty() { k } else { old.lo }, hi: k + 1 };
                }
            }
        }
        let tokens = build_tokens(&toks);
        let mut input = JavaInput {
            line_separator: newlines::guess_line_separator(text),
            text: text.to_string(),
            toks,
            tokens,
            kn,
            ranges,
            line_count,
            line_starts: newlines::line_offsets(text),
            position_tokens: Vec::new(),
            k_to_token: Vec::new(),
        };
        let mut position_tokens = Vec::new();
        let mut k_to_token = vec![0; (kn + 1) as usize];
        for (ti, token) in input.tokens.iter().enumerate() {
            let end = input.end_tok(ti);
            let mut upper = end.position;
            if !end.text.is_empty() {
                upper += end.text.len() - 1;
            }
            position_tokens.push((input.start_tok(ti).position, upper, ti));
            for &t in token.before.iter().chain(std::iter::once(&token.tok)).chain(token.after.iter()) {
                let idx = input.toks[t].index;
                if idx >= 0 {
                    k_to_token[idx as usize] = ti;
                }
            }
        }
        input.position_tokens = position_tokens;
        input.k_to_token = k_to_token;
        Ok(input)
    }

    pub fn tok(&self, i: usize) -> &Tok {
        &self.toks[i]
    }

    pub fn token_tok(&self, token: usize) -> &Tok {
        &self.toks[self.tokens[token].tok]
    }

    /// The earliest non-whitespace tok in the token.
    pub fn start_tok(&self, token: usize) -> &Tok {
        let t = &self.tokens[token];
        for &b in &t.before {
            if self.toks[b].index >= 0 {
                return &self.toks[b];
            }
        }
        &self.toks[t.tok]
    }

    /// The last non-whitespace tok in the token.
    pub fn end_tok(&self, token: usize) -> &Tok {
        let t = &self.tokens[token];
        for &a in t.after.iter().rev() {
            if self.toks[a].index >= 0 {
                return &self.toks[a];
            }
        }
        &self.toks[t.tok]
    }

    /// The token whose closed position range contains `position`.
    pub fn token_at(&self, position: usize) -> Option<usize> {
        let idx = self.position_tokens.partition_point(|&(lo, _, _)| lo <= position);
        if idx == 0 {
            return None;
        }
        let (lo, hi, t) = self.position_tokens[idx - 1];
        if lo <= position && position <= hi { Some(t) } else { None }
    }

    pub fn column_at(&self, position: usize) -> Option<usize> {
        // Toks are sorted by (distinct) position.
        let i = self.toks.binary_search_by_key(&position, |t| t.position).ok()?;
        Some(self.toks[i].column)
    }

    pub fn token_for_k(&self, k: i32) -> usize {
        self.k_to_token[k as usize]
    }

    /// 1-based line number of a byte position.
    pub fn line_number(&self, position: usize) -> usize {
        self.line_starts.partition_point(|&s| s <= position)
    }

    /// Converts byte ranges of the input into merged closed-open ranges of tok indices.
    pub fn character_ranges_to_token_ranges(&self, ranges: &[(usize, usize)]) -> Result<Vec<(i32, i32)>, LexError> {
        let mut result = Vec::new();
        for &(start, end) in ranges {
            if end > self.text.len() {
                // Same message as the original (whose FormatterException message already starts with "error: ").
                let length = end - start;
                return Err(LexError::new(format!(
                    "error: invalid length {length}, offset + length ({end}) is outside the file"
                )));
            }
            // An empty range means "format the line under the cursor".
            let end = if end == start { start + 1 } else { end };
            let enclosed: Vec<usize> = self
                .position_tokens
                .iter()
                .filter(|&&(lo, hi, _)| lo < end && hi >= start)
                .map(|&(_, _, t)| t)
                .collect();
            if let (Some(&first), Some(&last)) = (enclosed.first(), enclosed.last()) {
                let range = (self.token_tok(first).index, self.token_tok(last).index + 1);
                crate::output::add_closed_open(&mut result, range);
            }
        }
        Ok(result)
    }

    pub fn get_ranges(&self, line: usize) -> Range {
        self.ranges.get(line).copied().unwrap_or(Range::EMPTY)
    }
}
