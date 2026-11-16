//! Port of `JavaOutput.java`: collects the output lines, re-inserting blank lines from the input where wanted, and
//! computes the replacements that turn the input into the output for the requested token ranges.
//!
//! Throughout, `i` indexes input lines, `j` output lines and `k` toks.

use std::collections::HashMap;

use crate::doc::State;
use crate::input::{JavaInput, Range};
use crate::newlines;
use crate::ops::BlankLineWanted;

pub struct JavaOutput<'a> {
    input: &'a JavaInput,
    blank_lines: &'a HashMap<i32, BlankLineWanted>,
    partial_format_ranges: &'a [(i32, i32)],
    lines: Vec<String>,
    /// For each output line, the range of tok indices lying on it.
    ranges: Vec<Range>,
    i_line: usize,
    last_k: i32,
    spaces_pending: usize,
    newlines_pending: usize,
    line_builder: String,
}

/// A replacement of the byte range `start..end` of the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

fn is_whitespace(c: char) -> bool {
    c.is_whitespace()
}

impl<'a> JavaOutput<'a> {
    pub fn new(
        input: &'a JavaInput,
        blank_lines: &'a HashMap<i32, BlankLineWanted>,
        partial_format_ranges: &'a [(i32, i32)],
    ) -> JavaOutput<'a> {
        JavaOutput {
            input,
            blank_lines,
            partial_format_ranges,
            lines: Vec::new(),
            ranges: Vec::new(),
            i_line: 0,
            last_k: -1,
            spaces_pending: 0,
            newlines_pending: 0,
            line_builder: String::new(),
        }
    }

    pub fn append(&mut self, state: &State, text: &str, range: Range) {
        if !range.is_empty() {
            let mut saw_newlines = false;
            let i_n = self.input.line_count;
            while self.i_line < i_n
                && (self.input.get_ranges(self.i_line).is_empty() || self.input.get_ranges(self.i_line).hi <= range.lo)
            {
                if self.input.get_ranges(self.i_line).is_empty() {
                    saw_newlines = true;
                }
                self.i_line += 1;
            }
            let is_comment = text.starts_with("//") || text.starts_with("/*");
            let wanted = if is_comment {
                saw_newlines
            } else {
                match self.blank_lines.get(&self.last_k) {
                    Some(w) => w.wanted(state).unwrap_or(saw_newlines),
                    None => false,
                }
            };
            if wanted {
                self.newlines_pending += 1;
            }
        }
        if newlines::is_newline(text) {
            if self.newlines_pending == 0 {
                self.newlines_pending += 1;
            }
            self.spaces_pending = 0;
        } else {
            let mut ranges_set = false;
            let mut chars = text.chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    ' ' => self.spaces_pending += 1,
                    '\r' | '\n' => {
                        if c == '\r' && chars.peek() == Some(&'\n') {
                            chars.next();
                        }
                        self.spaces_pending = 0;
                        self.newlines_pending += 1;
                    }
                    _ => {
                        while self.newlines_pending > 0 {
                            if !self.lines.is_empty() || !self.line_builder.is_empty() {
                                self.lines.push(std::mem::take(&mut self.line_builder));
                            }
                            self.line_builder.clear();
                            ranges_set = false;
                            self.newlines_pending -= 1;
                        }
                        for _ in 0..self.spaces_pending {
                            self.line_builder.push(' ');
                        }
                        self.spaces_pending = 0;
                        self.line_builder.push(c);
                        if !range.is_empty() && !ranges_set {
                            let j = self.lines.len();
                            while self.ranges.len() <= j {
                                self.ranges.push(Range::EMPTY);
                            }
                            self.ranges[j] = self.ranges[j].union(range);
                            ranges_set = true;
                        }
                    }
                }
            }
        }
        if !range.is_empty() {
            self.last_k = range.hi;
        }
    }

    pub fn indent(&mut self, indent: i32) {
        self.spaces_pending = indent.max(0) as usize;
    }

    /// Flush any incomplete last line, then add the EOF token into the data structures.
    pub fn flush(&mut self) {
        let last = std::mem::take(&mut self.line_builder);
        if !last.chars().all(is_whitespace) {
            self.lines.push(last);
        }
        let j_n = self.lines.len();
        while self.ranges.len() < j_n {
            self.ranges.push(Range::EMPTY);
        }
        let kn = self.input.kn;
        self.ranges.push(Range { lo: kn, hi: kn + 1 });
    }

    fn get_ranges(&self, j: usize) -> Range {
        self.ranges.get(j).copied().unwrap_or(Range::EMPTY)
    }

    /// Map from tok index to the range of output lines it lies on (`InputOutput.makeKToIJ`).
    fn k_to_j(&self) -> HashMap<i32, (usize, usize)> {
        let mut map: HashMap<i32, (usize, usize)> = HashMap::new();
        for j in 0..=self.lines.len() {
            let r = self.get_ranges(j);
            for k in r.lo..r.hi.max(r.lo) {
                map.entry(k).and_modify(|e| e.1 = j + 1).or_insert((j, j + 1));
            }
        }
        map
    }

    /// Expands a token range to start and end on boundaries that can be formatted independently.
    fn expand_to_breakable_regions(&self, lo_tok: i32, hi_tok: i32) -> Option<(i32, i32)> {
        let containing = |k: i32| self.partial_format_ranges.iter().find(|&&(a, b)| a <= k && k <= b).copied();
        let lo = containing(lo_tok)?.0;
        let hi = containing(hi_tok)?.1;
        Some((lo, hi + 1))
    }

    /// The replacements converting the input into the output, for the given closed-open token ranges.
    pub fn format_replacements(&self, token_ranges: &[(i32, i32)]) -> Vec<Replacement> {
        let input = self.input;
        let text = input.text.as_str();
        let sep = input.line_separator;
        let k_to_j = self.k_to_j();
        // Expand the token ranges to formattable boundaries, merging connected ranges.
        let mut breakable: Vec<(i32, i32)> = Vec::new();
        for &(lo, hi) in token_ranges {
            let (lo, hi) = (lo.max(0), hi.min(input.kn + 1));
            if lo >= hi {
                continue;
            }
            if let Some(r) = self.expand_to_breakable_regions(lo, hi - 1) {
                add_closed_open(&mut breakable, r);
            }
        }
        let mut result = Vec::new();
        for (lo, hi) in breakable {
            let start_tok = input.start_tok(input.token_for_k(lo));
            let end_tok = input.end_tok(input.token_for_k(hi - 1));
            let mut replacement = String::new();
            let mut replace_from = start_tok.position;
            // Replace leading whitespace in the input with the whitespace from the formatted file.
            while replace_from > 0 {
                let previous = text[..replace_from].chars().next_back().unwrap();
                if !is_whitespace(previous) {
                    break;
                }
                replace_from -= previous.len_utf8();
            }
            let (Some(&(start_j, _)), Some(&(_, end_j))) = (k_to_j.get(&start_tok.index), k_to_j.get(&end_tok.index))
            else {
                continue;
            };
            let mut i = start_j;
            // Include leading blank lines from the formatted output, unless the range starts the file.
            while i > 0 && self.lines.get(i - 1).is_some_and(|l| l.is_empty()) {
                i -= 1;
            }
            while i < end_j {
                if i < self.lines.len() {
                    if i > 0 {
                        replacement.push_str(sep);
                    }
                    replacement.push_str(&self.lines[i]);
                }
                i += 1;
            }
            let mut replace_to = (end_tok.position + end_tok.text.len()).min(text.len());
            // If the range ended in the trailing trivia of the last token before EOF, format up to EOF.
            if end_tok.index == input.kn - 1 {
                replace_to = text.len();
            }
            // Replace trailing whitespace with the formatted whitespace, but keep the indentation of the next line
            // if the trailing whitespace contains a line break.
            let mut newline = None;
            while replace_to < text.len() {
                let next = text[replace_to..].chars().next().unwrap();
                if !is_whitespace(next) {
                    break;
                }
                match newlines::has_newline_at(text, replace_to) {
                    Some(len) => {
                        newline = Some(replace_to);
                        replace_to += len;
                    }
                    None => replace_to += next.len_utf8(),
                }
            }
            if let Some(nl) = newline {
                replace_to = nl;
            } else {
                replacement.push_str(sep);
            }
            while i < self.lines.len() {
                let after = &self.lines[i];
                match after.find(|c: char| !is_whitespace(c)) {
                    None => replacement.push_str(sep),
                    Some(idx) => {
                        if newline.is_none() {
                            replacement.push_str(&after[..idx]);
                        }
                        break;
                    }
                }
                i += 1;
            }
            result.push(Replacement { start: replace_from, end: replace_to, text: replacement });
        }
        result
    }
}

/// Adds a closed-open range to a sorted set of ranges, merging connected (overlapping or touching) ones.
pub fn add_closed_open(ranges: &mut Vec<(i32, i32)>, (mut lo, mut hi): (i32, i32)) {
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
