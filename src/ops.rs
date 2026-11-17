// Derived from palantir-java-format's OpsBuilder.java
// Copyright 2015-2019 Google Inc.; Copyright 2019-2025 Palantir Technologies Inc.
// Modified by the jtk authors: ported to Rust. Licensed under the Apache License, Version 2.0.

//! Port of `OpsBuilder.java`: builds the linear op stream and splices in comments.

use std::collections::HashMap;

use crate::doc::{BreakDoc, BreakTag, FillMode, Indent, Op, OpenOp, RealOrImaginary, State, TokenDoc};
use crate::input::JavaInput;

/// A request to add or remove a blank line in the output.
#[derive(Clone, Debug)]
pub enum BlankLineWanted {
    Yes,
    No,
    Preserve,
    Conditional(Vec<BreakTag>),
}

impl BlankLineWanted {
    pub fn wanted(&self, state: &State) -> Option<bool> {
        match self {
            BlankLineWanted::Yes => Some(true),
            BlankLineWanted::No => Some(false),
            BlankLineWanted::Preserve => None,
            BlankLineWanted::Conditional(tags) => {
                if tags.iter().any(|&t| state.was_break_taken(t)) {
                    Some(true)
                } else {
                    None
                }
            }
        }
    }

    fn merge(self, other: BlankLineWanted) -> BlankLineWanted {
        match (self, other) {
            (BlankLineWanted::Conditional(mut a), BlankLineWanted::Conditional(b)) => {
                a.extend(b);
                BlankLineWanted::Conditional(a)
            }
            (BlankLineWanted::Conditional(_), other) => other,
            (this, _) => this,
        }
    }
}

pub struct OpsOutput {
    pub ops: Vec<Op>,
    pub blank_lines: HashMap<i32, BlankLineWanted>,
    /// Closed ranges of tok indices that can be formatted independently, sorted.
    pub partial_format_ranges: Vec<(i32, i32)>,
}

pub struct OpsBuilder<'a> {
    pub input: &'a JavaInput,
    ops: Vec<Op>,
    blank_lines: HashMap<i32, BlankLineWanted>,
    token_i: usize,
    input_position: i64,
    depth: i32,
    next_tag: BreakTag,
    last_partial_format_boundary: Option<usize>,
    partial_format_ranges: Vec<(i32, i32)>,
    pub error: Option<crate::formatter::FormatError>,
}

impl<'a> OpsBuilder<'a> {
    pub fn new(input: &'a JavaInput) -> OpsBuilder<'a> {
        OpsBuilder {
            input,
            ops: Vec::new(),
            blank_lines: HashMap::new(),
            token_i: 0,
            input_position: i64::MIN,
            depth: 0,
            next_tag: 0,
            last_partial_format_boundary: None,
            partial_format_ranges: Vec::new(),
            error: None,
        }
    }

    pub fn new_tag(&mut self) -> BreakTag {
        self.next_tag += 1;
        self.next_tag
    }

    fn fail(&mut self, message: String) {
        if self.error.is_none() {
            let position = self.input_position.clamp(0, self.input.text.len() as i64) as usize;
            self.error = Some(crate::formatter::FormatError::at_offset(&self.input.text, position, message));
        }
    }

    fn add(&mut self, op: Op) {
        match op {
            Op::Open(_) => self.depth += 1,
            Op::Close => {
                self.depth -= 1;
                if self.depth < 0 {
                    self.fail("unbalanced close".into());
                }
            }
            _ => {}
        }
        self.ops.push(op);
    }

    pub fn add_all(&mut self, ops: Vec<Op>) {
        for op in ops {
            self.add(op);
        }
    }

    pub fn depth(&self) -> i32 {
        self.depth
    }

    /// Return the actual size of the AST node at position, including comments.
    pub fn actual_size(&self, position: usize, length: usize) -> usize {
        let input = self.input;
        let Some(start_token) = input.token_at(position) else {
            return length;
        };
        let mut start = input.token_tok(start_token).position;
        for &t in &input.tokens[start_token].before {
            let tok = input.tok(t);
            if tok.is_comment() {
                start = start.min(tok.position);
            }
        }
        let Some(end_token) = input.token_at((position + length).saturating_sub(1)) else {
            return length;
        };
        let end_tok = input.token_tok(end_token);
        let mut end = end_tok.position + end_tok.text.len();
        for &t in &input.tokens[end_token].after {
            let tok = input.tok(t);
            if tok.is_comment() {
                end = end.max(tok.position + tok.text.len());
            }
        }
        end.saturating_sub(start)
    }

    /// Return the start position of the token at `position`, including leading comments on the same line.
    pub fn actual_start_column(&self, position: usize) -> usize {
        let input = self.input;
        let Some(start_token) = input.token_at(position) else {
            return position;
        };
        let mut start = input.token_tok(start_token).position;
        let line0 = input.line_number(start);
        for &t in &input.tokens[start_token].before {
            let tok = input.tok(t);
            if line0 != input.line_number(tok.position) {
                return start;
            }
            if tok.is_comment() {
                start = start.min(tok.position);
            }
        }
        start
    }

    pub fn sync(&mut self, position: usize) {
        let position = position as i64;
        if position > self.input_position {
            self.input_position = position;
            let tokens = &self.input.tokens;
            if self.token_i < tokens.len() && position > self.input.token_tok(self.token_i).position as i64 {
                let text = self.input.token_tok(self.token_i).text.clone();
                self.token_i += 1;
                self.fail(format!("did not generate token \"{text}\""));
            }
        }
    }

    /// Output any remaining tokens from the input stream (e.g. terminal whitespace).
    pub fn drain(&mut self) {
        let input_position = self.input.text.len() as i64 + 1;
        if input_position > self.input_position {
            while self.token_i < self.input.tokens.len()
                && input_position > self.input.token_tok(self.token_i).position as i64
            {
                let token = self.token_i;
                self.token_i += 1;
                self.add(Op::Token(TokenDoc {
                    token,
                    real_or_imaginary: RealOrImaginary::Imaginary,
                    plus_indent_comments_before: Indent::ZERO,
                    break_and_indent_trailing_comment: None,
                }));
            }
        }
        self.input_position = input_position;
        if self.depth != 0 {
            self.fail(format!("saw {} unclosed ops", self.depth));
        }
    }

    pub fn open(&mut self, plus_indent: Indent) {
        self.add(Op::Open(OpenOp::new(plus_indent)));
    }

    pub fn open_op(&mut self, op: OpenOp) {
        self.add(Op::Open(op));
    }

    pub fn close(&mut self) {
        self.add(Op::Close);
    }

    pub fn peek_token(&self) -> Option<&'a str> {
        self.peek_token_skip(0)
    }

    pub fn peek_is(&self, s: &str) -> bool {
        self.peek_token() == Some(s)
    }

    pub fn peek_token_skip(&self, skip: usize) -> Option<&'a str> {
        let input: &'a JavaInput = self.input;
        let idx = self.token_i + skip;
        if idx < input.tokens.len() { Some(input.token_tok(idx).text.as_str()) } else { None }
    }

    /// Return whether the last token emitted is followed by a newline.
    pub fn most_recent_token_followed_by_newline(&self) -> bool {
        if self.token_i == 0 {
            return false;
        }
        self.input.tokens[self.token_i - 1].after.iter().any(|&t| self.input.tok(t).is_newline())
    }

    pub fn guess_token(&mut self, token: &str) {
        self.token(token, RealOrImaginary::Imaginary, Indent::ZERO, None);
    }

    pub fn token(
        &mut self,
        token: &str,
        real_or_imaginary: RealOrImaginary,
        plus_indent_comments_before: Indent,
        break_and_indent_trailing_comment: Option<Indent>,
    ) {
        if self.peek_token() == Some(token) {
            let t = self.token_i;
            self.token_i += 1;
            self.add(Op::Token(TokenDoc {
                token: t,
                real_or_imaginary: RealOrImaginary::Real,
                plus_indent_comments_before,
                break_and_indent_trailing_comment,
            }));
        } else if real_or_imaginary == RealOrImaginary::Real {
            let peek = self.peek_token().unwrap_or("<EOF>").to_string();
            self.fail(format!("expected token: '{peek}'; generated {token} instead"));
        }
    }

    /// Emit a single- or multi-character op by breaking it into single-character tokens.
    pub fn op(&mut self, op: &str) {
        for i in 0..op.len() {
            self.token(&op[i..i + 1], RealOrImaginary::Real, Indent::ZERO, None);
        }
    }

    pub fn space(&mut self) {
        self.add(Op::Space);
    }

    pub fn break_op(&mut self) {
        self.break_full(FillMode::Unified, "", Indent::ZERO, None);
    }

    pub fn break_indent(&mut self, plus_indent: Indent) {
        self.break_full(FillMode::Unified, "", plus_indent, None);
    }

    pub fn break_flat(&mut self, flat: &'static str) {
        self.break_full(FillMode::Unified, flat, Indent::ZERO, None);
    }

    pub fn break_to_fill(&mut self, flat: &'static str) {
        self.break_full(FillMode::Independent, flat, Indent::ZERO, None);
    }

    pub fn forced_break(&mut self) {
        self.break_full(FillMode::Forced, "", Indent::ZERO, None);
    }

    pub fn break_full(&mut self, fill_mode: FillMode, flat: &'static str, plus_indent: Indent, tag: Option<BreakTag>) {
        self.add(Op::Break(BreakDoc::new(fill_mode, flat, plus_indent, tag)));
    }

    pub fn break_doc(&mut self, b: BreakDoc) {
        self.add(Op::Break(b));
    }

    /// Marks the boundary of a region that can be partially formatted. The boundary is included in the following
    /// region: `[boundary0, boundary1), [boundary1, boundary2), ...`.
    pub fn mark_for_partial_format(&mut self) {
        let Some(last) = self.last_partial_format_boundary else {
            self.last_partial_format_boundary = Some(self.token_i);
            return;
        };
        if self.token_i == last {
            return;
        }
        let lo = self.input.start_tok(last).index;
        let hi = self.input.end_tok(self.token_i - 1).index;
        add_closed_range(&mut self.partial_format_ranges, lo, hi);
        self.last_partial_format_boundary = Some(self.token_i);
    }

    pub fn blank_line_wanted(&mut self, wanted: BlankLineWanted) {
        let Some(token) = self.input.tokens.get(self.token_i) else {
            return;
        };
        let k = token
            .before
            .iter()
            .map(|&t| self.input.tok(t).index)
            .find(|&i| i >= 0)
            .unwrap_or_else(|| self.input.tok(token.tok).index);
        self.blank_line(k, wanted);
    }

    fn blank_line(&mut self, k: i32, wanted: BlankLineWanted) {
        match self.blank_lines.remove(&k) {
            Some(existing) => {
                self.blank_lines.insert(k, existing.merge(wanted));
            }
            None => {
                self.blank_lines.insert(k, wanted);
            }
        }
    }

    /// Build the final list of ops, splicing in comments.
    pub fn build(mut self) -> Result<OpsOutput, crate::formatter::FormatError> {
        self.mark_for_partial_format();
        if let Some(e) = self.error.take() {
            return Err(e);
        }
        let input = self.input;
        let ops = std::mem::take(&mut self.ops);
        let ops_n = ops.len();
        let mut tok_ops: Vec<Vec<Op>> = vec![Vec::new(); ops_n + 1];
        let make_comment = |tok: usize, v: &mut Vec<Op>| {
            v.push(Op::Comment(tok));
            if !input.tok(tok).is_slash_star_comment() {
                v.push(Op::Break(BreakDoc::forced()));
            }
        };
        for i in 0..ops_n {
            let Op::Token(token_op) = &ops[i] else {
                continue;
            };
            let token = &input.tokens[token_op.token];
            let mut j = i;
            while j > 0 && matches!(ops[j - 1], Op::Open(_)) {
                j -= 1;
            }
            let mut k = i;
            while k + 1 < ops_n && matches!(ops[k + 1], Op::Close) {
                k += 1;
            }
            if token_op.real_or_imaginary == RealOrImaginary::Real {
                let mut newlines = 0;
                let mut space = false;
                let mut last_was_comment = false;
                let mut allow_blank_after_last_comment = false;
                for &tb in &token.before {
                    let tok_before = input.tok(tb);
                    if tok_before.is_newline() {
                        newlines += 1;
                    } else if tok_before.is_comment() {
                        tok_ops[j].push(Op::Break(BreakDoc::new(
                            if tok_before.is_slash_slash_comment() { FillMode::Forced } else { FillMode::Unified },
                            "",
                            token_op.plus_indent_comments_before.clone(),
                            None,
                        )));
                        make_comment(tb, &mut tok_ops[j]);
                        space = tok_before.is_slash_star_comment();
                        newlines = 0;
                        last_was_comment = true;
                        if tok_before.is_javadoc_comment() {
                            tok_ops[j].push(Op::Break(BreakDoc::forced()));
                        }
                        allow_blank_after_last_comment = tok_before.is_slash_slash_comment()
                            || (tok_before.is_slash_star_comment() && !tok_before.is_javadoc_comment());
                    }
                }
                if allow_blank_after_last_comment && newlines > 1 {
                    let idx = input.tok(token.tok).index;
                    self.blank_line(idx, BlankLineWanted::Yes);
                }
                if last_was_comment && newlines > 0 {
                    tok_ops[j].push(Op::Break(BreakDoc::forced()));
                } else if space {
                    tok_ops[j].push(Op::Space);
                }
                let non_nls_comments_after_plus = token.after.iter().any(|&t| {
                    let tok = input.tok(t);
                    tok.is_slash_slash_comment() && tok.text.contains("$NON-NLS")
                }) && input.tok(token.tok).text == "+"
                    && k > 0
                    && matches!(ops[k - 1], Op::Break(_));
                let tok_after_pos = if non_nls_comments_after_plus { k - 1 } else { k + 1 };
                for &ta in &token.after {
                    let tok_after = input.tok(ta);
                    if tok_after.is_comment() {
                        let break_after = tok_after.is_javadoc_comment()
                            || (tok_after.is_slash_star_comment()
                                && token_op.break_and_indent_trailing_comment.is_some());
                        if break_after {
                            tok_ops[tok_after_pos].push(Op::Break(BreakDoc::new(
                                FillMode::Forced,
                                "",
                                token_op.break_and_indent_trailing_comment.clone().unwrap_or(Indent::ZERO),
                                None,
                            )));
                        } else {
                            tok_ops[tok_after_pos].push(Op::Space);
                        }
                        make_comment(ta, &mut tok_ops[tok_after_pos]);
                        if break_after {
                            tok_ops[tok_after_pos].push(Op::Break(BreakDoc::forced()));
                        }
                    }
                }
            } else {
                let mut newlines = 0;
                let mut last_was_comment = false;
                for &tb in &token.before {
                    let tok_before = input.tok(tb);
                    if tok_before.is_newline() {
                        newlines += 1;
                    } else if tok_before.is_comment() {
                        newlines = 0;
                        last_was_comment = true;
                    }
                    if last_was_comment && newlines > 0 {
                        tok_ops[j].push(Op::Break(BreakDoc::forced()));
                    }
                    tok_ops[j].push(Op::Comment(tb));
                }
                for &ta in &token.after {
                    tok_ops[k + 1].push(Op::Comment(ta));
                }
            }
        }
        let is_forced = |op: &Op| matches!(op, Op::Break(b) if b.is_forced());
        let mut new_ops = Vec::with_capacity(ops_n * 2);
        let mut after_forced_break = false;
        let starting = State::starting();
        for (i, op) in ops.into_iter().enumerate() {
            for op in std::mem::take(&mut tok_ops[i]) {
                if !(after_forced_break && matches!(op, Op::Space)) {
                    after_forced_break = is_forced(&op);
                    new_ops.push(op);
                }
            }
            if after_forced_break {
                let skip = match &op {
                    Op::Space => true,
                    Op::Break(b) => b.plus_indent.eval(&starting) == 0 && b.flat == " ",
                    _ => false,
                };
                if skip {
                    continue;
                }
            }
            if !matches!(op, Op::Open(_)) {
                after_forced_break = is_forced(&op);
            }
            new_ops.push(op);
        }
        for op in std::mem::take(&mut tok_ops[ops_n]) {
            if !(after_forced_break && matches!(op, Op::Space)) {
                after_forced_break = is_forced(&op);
                new_ops.push(op);
            }
        }
        Ok(OpsOutput {
            ops: new_ops,
            blank_lines: self.blank_lines,
            partial_format_ranges: std::mem::take(&mut self.partial_format_ranges),
        })
    }
}

/// Adds a closed range to a sorted set of closed ranges, merging overlapping ones (Guava `TreeRangeSet` semantics:
/// `[1, 3]` and `[4, 6]` stay separate). Ranges are produced in increasing order, so usually only the last one can
/// overlap.
fn add_closed_range(ranges: &mut Vec<(i32, i32)>, mut lo: i32, mut hi: i32) {
    if ranges.last().is_none_or(|&(_, b)| b < lo) {
        ranges.push((lo, hi));
        return;
    }
    ranges.retain(|&(a, b)| {
        if a <= hi && lo <= b {
            lo = lo.min(a);
            hi = hi.max(b);
            false
        } else {
            true
        }
    });
    let idx = ranges.partition_point(|&(a, _)| a < lo);
    ranges.insert(idx, (lo, hi));
}
