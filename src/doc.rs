//! The document model and the line-breaking algorithm (port of the `doc` package: `Doc`, `Level`, `Break`, `Token`,
//! `Comment`, `NonBreakingSpace`, `State`, `DocBuilder` and friends).
//!
//! Docs live in an arena ([`Docs`]) and are referred to by [`DocId`]. The [`State`] uses persistent maps so that
//! alternative layouts can be explored cheaply, like the original's functional-java collections.

use std::cell::OnceCell;
use std::rc::Rc;

use std::hash::{BuildHasherDefault, Hasher};

/// A multiplicative hasher for the small integer ids (docs, break tags) used as keys in [`State`]; much cheaper than
/// the default SipHash.
#[derive(Default)]
pub struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
        }
    }
    fn write_usize(&mut self, n: usize) {
        self.0 = (n as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

type IdBuild = BuildHasherDefault<IdHasher>;
type PMap<K, V> = im_rc::HashMap<K, V, IdBuild>;
type PSet<K> = im_rc::HashSet<K, IdBuild>;

use crate::comments::CommentsHelper;
use crate::input::{JavaInput, Range};
use crate::newlines::{self, jlen};
use crate::output::JavaOutput;

pub type DocId = usize;
pub type BreakTag = usize;

pub const TEXT_BLOCK_DELIMITER: &str = "\"\"\"";

/// An indent: a constant, or conditional on whether a break tag was taken.
#[derive(Clone, Debug, PartialEq)]
pub enum Indent {
    Const(i32),
    If(BreakTag, Rc<Indent>, Rc<Indent>),
}

impl Indent {
    pub const ZERO: Indent = Indent::Const(0);

    pub fn if_(tag: BreakTag, then: Indent, otherwise: Indent) -> Indent {
        Indent::If(tag, Rc::new(then), Rc::new(otherwise))
    }

    pub fn eval(&self, state: &State) -> i32 {
        match self {
            Indent::Const(n) => *n,
            Indent::If(tag, t, e) => {
                if state.was_break_taken(*tag) {
                    t.eval(state)
                } else {
                    e.eval(state)
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillMode {
    Unified,
    Independent,
    Forced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakBehaviour {
    BreakThisLevel,
    PreferBreakingLastInnerLevel { keep_indent_when_inlined: bool },
    InlineSuffix,
    BreakOnlyIfInnerLevelsThenFitOnOneLine { keep_indent_when_inlined: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LastLevelBreakability {
    Abort,
    AcceptInlineChain,
    CheckInner,
    AcceptInlineChainIfSimpleOtherwiseCheckInner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartialInlineability {
    AlwaysPartiallyInlineable,
    IfFirstLevelFits,
}

#[derive(Clone, Debug)]
pub struct OpenOp {
    pub plus_indent: Indent,
    pub break_behaviour: BreakBehaviour,
    pub breakability_if_last_level: LastLevelBreakability,
    pub partial_inlineability: PartialInlineability,
    pub simple: bool,
    pub debug_name: Option<&'static str>,
    pub column_limit_before_last_break: Option<i32>,
}

impl OpenOp {
    pub fn new(plus_indent: Indent) -> OpenOp {
        OpenOp {
            plus_indent,
            break_behaviour: BreakBehaviour::BreakThisLevel,
            breakability_if_last_level: LastLevelBreakability::Abort,
            partial_inlineability: PartialInlineability::AlwaysPartiallyInlineable,
            simple: true,
            debug_name: None,
            column_limit_before_last_break: None,
        }
    }
    pub fn behaviour(mut self, b: BreakBehaviour) -> Self {
        self.break_behaviour = b;
        self
    }
    pub fn breakability(mut self, b: LastLevelBreakability) -> Self {
        self.breakability_if_last_level = b;
        self
    }
    pub fn partial(mut self, p: PartialInlineability) -> Self {
        self.partial_inlineability = p;
        self
    }
    pub fn simple(mut self, s: bool) -> Self {
        self.simple = s;
        self
    }
    pub fn name(mut self, n: &'static str) -> Self {
        self.debug_name = Some(n);
        self
    }
    pub fn column_limit(mut self, limit: i32) -> Self {
        self.column_limit_before_last_break = Some(limit);
        self
    }
}

#[derive(Clone, Debug)]
pub struct BreakDoc {
    pub fill_mode: FillMode,
    pub flat: &'static str,
    pub plus_indent: Indent,
    pub opt_tag: Option<BreakTag>,
    pub has_column_limit: bool,
}

impl BreakDoc {
    pub fn new(fill_mode: FillMode, flat: &'static str, plus_indent: Indent, opt_tag: Option<BreakTag>) -> BreakDoc {
        BreakDoc { fill_mode, flat, plus_indent, opt_tag, has_column_limit: false }
    }
    pub fn forced() -> BreakDoc {
        BreakDoc::new(FillMode::Forced, "", Indent::ZERO, None)
    }
    pub fn is_forced(&self) -> bool {
        self.fill_mode == FillMode::Forced
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RealOrImaginary {
    Real,
    Imaginary,
}

#[derive(Clone, Debug)]
pub struct TokenDoc {
    /// Index into [`JavaInput::tokens`].
    pub token: usize,
    pub real_or_imaginary: RealOrImaginary,
    pub plus_indent_comments_before: Indent,
    pub break_and_indent_trailing_comment: Option<Indent>,
}

/// An op in the linear op stream produced by the visitor.
#[derive(Clone, Debug)]
pub enum Op {
    Open(OpenOp),
    Close,
    Token(TokenDoc),
    Break(BreakDoc),
    Space,
    /// A comment (or other non-token tok), by tok index.
    Comment(usize),
}

pub struct LevelDoc {
    pub open: OpenOp,
    pub docs: Vec<DocId>,
    splits: OnceCell<Rc<SplitsBreaks>>,
    max_depth: OnceCell<i32>,
}

pub enum DocKind {
    Level(LevelDoc),
    Break(BreakDoc),
    Token(TokenDoc),
    Comment(usize),
    Space,
}

pub struct Doc {
    pub kind: DocKind,
    width: OnceCell<f32>,
    flat: OnceCell<Rc<str>>,
    range: OnceCell<Range>,
    starts_with_break: OnceCell<StartsWithBreak>,
}

pub struct SplitsBreaks {
    pub splits: Vec<Vec<DocId>>,
    pub breaks: Vec<DocId>,
}

/// The arena holding all docs.
pub struct Docs<'a> {
    pub docs: Vec<Doc>,
    pub input: &'a JavaInput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StartsWithBreak {
    Empty,
    No,
    Yes,
}

impl<'a> Docs<'a> {
    /// Build the doc tree from a list of ops (port of `DocBuilder`). Returns the arena and the root level.
    pub fn build(input: &'a JavaInput, ops: Vec<Op>) -> (Docs<'a>, DocId) {
        let mut docs = Docs { docs: Vec::new(), input };
        let root = docs.push(DocKind::Level(LevelDoc {
            open: OpenOp::new(Indent::ZERO).name("root"),
            docs: Vec::new(),
            splits: OnceCell::new(),
            max_depth: OnceCell::new(),
        }));
        let mut stack = vec![root];
        let mut append_level = root;
        for op in ops {
            match op {
                Op::Open(open) => {
                    let level = docs.push(DocKind::Level(LevelDoc {
                        open,
                        docs: Vec::new(),
                        splits: OnceCell::new(),
                        max_depth: OnceCell::new(),
                    }));
                    stack.push(level);
                }
                Op::Close => {
                    let top = stack.pop().unwrap();
                    let parent = *stack.last().unwrap();
                    docs.level_mut(parent).docs.push(top);
                }
                Op::Break(b) => {
                    append_level = *stack.last().unwrap();
                    let id = docs.push(DocKind::Break(b));
                    docs.level_mut(append_level).docs.push(id);
                }
                Op::Token(t) => {
                    let id = docs.push(DocKind::Token(t));
                    docs.level_mut(append_level).docs.push(id);
                }
                Op::Comment(c) => {
                    let id = docs.push(DocKind::Comment(c));
                    docs.level_mut(append_level).docs.push(id);
                }
                Op::Space => {
                    let id = docs.push(DocKind::Space);
                    docs.level_mut(append_level).docs.push(id);
                }
            }
        }
        (docs, root)
    }

    fn push(&mut self, kind: DocKind) -> DocId {
        self.docs.push(Doc {
            kind,
            width: OnceCell::new(),
            flat: OnceCell::new(),
            range: OnceCell::new(),
            starts_with_break: OnceCell::new(),
        });
        self.docs.len() - 1
    }

    fn level_mut(&mut self, id: DocId) -> &mut LevelDoc {
        match &mut self.docs[id].kind {
            DocKind::Level(l) => l,
            _ => unreachable!(),
        }
    }

    pub fn level(&self, id: DocId) -> &LevelDoc {
        match &self.docs[id].kind {
            DocKind::Level(l) => l,
            _ => unreachable!(),
        }
    }

    fn as_level(&self, id: DocId) -> Option<&LevelDoc> {
        match &self.docs[id].kind {
            DocKind::Level(l) => Some(l),
            _ => None,
        }
    }

    fn as_break(&self, id: DocId) -> Option<&BreakDoc> {
        match &self.docs[id].kind {
            DocKind::Break(b) => Some(b),
            _ => None,
        }
    }

    fn is_level(&self, id: DocId) -> bool {
        matches!(self.docs[id].kind, DocKind::Level(_))
    }

    fn token_text(&self, t: &TokenDoc) -> &str {
        &self.input.token_tok(t.token).text
    }

    pub fn width(&self, id: DocId) -> f32 {
        *self.docs[id].width.get_or_init(|| self.compute_width(id))
    }

    fn compute_width(&self, id: DocId) -> f32 {
        match &self.docs[id].kind {
            DocKind::Level(l) => l.docs.iter().map(|&d| self.width(d)).fold(0.0f32, |a, b| a + b),
            DocKind::Break(b) => {
                if b.is_forced() {
                    f32::INFINITY
                } else {
                    b.flat.len() as f32
                }
            }
            DocKind::Token(t) => {
                let text = self.token_text(t);
                if text.starts_with(TEXT_BLOCK_DELIMITER) {
                    TEXT_BLOCK_DELIMITER.len() as f32
                } else {
                    jlen(text) as f32
                }
            }
            DocKind::Comment(tok) => {
                let tok = self.input.tok(*tok);
                let idx = newlines::first_break(&tok.text);
                if tok.is_comment() {
                    match idx {
                        Some(i) if i > 0 => jlen(&tok.text[..i]) as f32,
                        _ => {
                            if tok.is_slash_slash_comment() && !tok.text.starts_with("// ") {
                                (tok.length() + 1) as f32
                            } else {
                                tok.length() as f32
                            }
                        }
                    }
                } else if idx.is_some() {
                    f32::INFINITY
                } else {
                    tok.length() as f32
                }
            }
            DocKind::Space => 1.0,
        }
    }

    pub fn flat(&self, id: DocId) -> Rc<str> {
        self.docs[id].flat.get_or_init(|| self.compute_flat(id)).clone()
    }

    fn compute_flat(&self, id: DocId) -> Rc<str> {
        match &self.docs[id].kind {
            DocKind::Level(l) => {
                let mut s = String::new();
                for &d in &l.docs {
                    s.push_str(&self.flat(d));
                }
                s.into()
            }
            DocKind::Break(b) => b.flat.into(),
            DocKind::Token(t) => self.token_text(t).into(),
            DocKind::Comment(tok) => comment_flat(&self.input.tok(*tok).text).into(),
            DocKind::Space => " ".into(),
        }
    }

    fn range(&self, id: DocId) -> Range {
        *self.docs[id].range.get_or_init(|| match &self.docs[id].kind {
            DocKind::Level(l) => l.docs.iter().fold(Range::EMPTY, |r, &d| r.union(self.range(d))),
            DocKind::Break(_) | DocKind::Space => Range::EMPTY,
            DocKind::Token(t) => Range::singleton(self.input.token_tok(t.token).index),
            DocKind::Comment(tok) => Range::singleton(self.input.tok(*tok).index),
        })
    }

    fn sum_width(&self, docs: &[DocId]) -> f32 {
        docs.iter().map(|&d| self.width(d)).fold(0.0f32, |a, b| a + b)
    }

    fn splits(&self, level: DocId) -> Rc<SplitsBreaks> {
        self.level(level).splits.get_or_init(|| Rc::new(self.split_by_breaks(&self.level(level).docs))).clone()
    }

    fn split_by_breaks(&self, docs: &[DocId]) -> SplitsBreaks {
        let mut splits = Vec::new();
        let mut breaks = Vec::new();
        let mut current = Vec::new();
        for &d in docs {
            if self.as_break(d).is_some() {
                splits.push(std::mem::take(&mut current));
                breaks.push(d);
            } else {
                current.push(d);
            }
        }
        splits.push(current);
        SplitsBreaks { splits, breaks }
    }

    fn max_depth(&self, level: DocId) -> i32 {
        *self.level(level).max_depth.get_or_init(|| {
            let l = self.level(level);
            let mut max_child = 0;
            for &d in &l.docs {
                if self.is_level(d) {
                    max_child = max_child.max(self.max_depth(d));
                }
            }
            let has_breaks = l.docs.iter().any(|&d| self.as_break(d).is_some());
            if has_breaks { 1 + max_child } else { max_child }
        })
    }

    /// Port of `StartsWithBreakVisitor` (memoized: docs are immutable once built).
    fn starts_with_break(&self, id: DocId) -> StartsWithBreak {
        *self.docs[id].starts_with_break.get_or_init(|| match &self.docs[id].kind {
            DocKind::Break(_) => StartsWithBreak::Yes,
            DocKind::Level(l) => l
                .docs
                .iter()
                .map(|&d| self.starts_with_break(d))
                .find(|&r| r != StartsWithBreak::Empty)
                .unwrap_or(StartsWithBreak::Empty),
            _ => StartsWithBreak::No,
        })
    }

    /// Port of `CountWidthUntilBreakVisitor`.
    fn count_width_until_break(&self, id: DocId, available_width: i32) -> f32 {
        let Some(level) = self.as_level(id) else {
            return self.width(id);
        };
        if level.open.partial_inlineability == PartialInlineability::IfFirstLevelFits
            && self.width(level.docs[0]) <= available_width as f32
        {
            return self.width(level.docs[0]);
        }
        let mut found = None;
        for (idx, &d) in level.docs.iter().enumerate() {
            if !self.is_level(d) {
                break;
            }
            if self.starts_with_break(d) != StartsWithBreak::Empty {
                found = Some(idx);
                break;
            }
        }
        if let Some(idx) = found {
            return self.count_width_until_break(level.docs[idx], available_width);
        }
        level
            .docs
            .iter()
            .take_while(|&&d| self.as_break(d).is_none())
            .map(|&d| self.width(d))
            .fold(0.0f32, |a, b| a + b)
    }

    fn non_empty_inner_levels(&self, level: DocId) -> Vec<DocId> {
        self.level(level)
            .docs
            .iter()
            .copied()
            .filter(|&d| self.is_level(d) && self.starts_with_break(d) != StartsWithBreak::Empty)
            .collect()
    }

    // ----------------------------------------------------------------------------------------------------------
    // Writing
    // ----------------------------------------------------------------------------------------------------------

    pub fn write(&self, id: DocId, state: &State, output: &mut JavaOutput) {
        match &self.docs[id].kind {
            DocKind::Level(_) => {
                if state.is_one_line(id) {
                    output.append(state, &self.flat(id), self.range(id));
                } else {
                    let sb = self.splits(id);
                    for &d in &sb.splits[0] {
                        self.write(d, state, output);
                    }
                    for i in 0..sb.breaks.len() {
                        self.write(sb.breaks[i], state, output);
                        for &d in &sb.splits[i + 1] {
                            self.write(d, state, output);
                        }
                    }
                }
            }
            DocKind::Break(b) => {
                let (broken, new_indent) = state.break_state(id);
                if broken {
                    output.append(state, "\n", Range::EMPTY);
                    output.indent(new_indent);
                } else {
                    output.append(state, b.flat, Range::EMPTY);
                }
            }
            DocKind::Token(t) => output.append(state, self.token_text(t), self.range(id)),
            DocKind::Comment(_) => {
                let text = state.tok_text(id).unwrap_or_else(|| self.flat(id));
                output.append(state, &text, self.range(id));
            }
            DocKind::Space => output.append(state, " ", Range::EMPTY),
        }
    }
}

pub fn comment_flat(text: &str) -> String {
    if text.starts_with("//") && !text.starts_with("// ") { format!("// {}", &text[2..]) } else { text.to_string() }
}

// --------------------------------------------------------------------------------------------------------------
// State
// --------------------------------------------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct State {
    pub last_indent: i32,
    pub indent: i32,
    pub column: i32,
    pub must_break: bool,
    pub num_lines: i32,
    pub branching_coefficient: i32,
    break_tags_taken: PSet<BreakTag>,
    break_states: PMap<DocId, i32>,
    level_states: PMap<DocId, bool>,
    tok_states: PMap<DocId, Rc<str>>,
}

impl State {
    pub fn starting() -> State {
        State::default()
    }

    /// `(broken, newIndent)`.
    pub fn break_state(&self, brk: DocId) -> (bool, i32) {
        match self.break_states.get(&brk) {
            Some(&indent) => (true, indent),
            None => (false, -1),
        }
    }

    pub fn was_break_taken(&self, tag: BreakTag) -> bool {
        self.break_tags_taken.contains(&tag)
    }

    pub fn is_one_line(&self, level: DocId) -> bool {
        self.level_states.get(&level).copied().unwrap_or(false)
    }

    fn tok_text(&self, comment: DocId) -> Option<Rc<str>> {
        self.tok_states.get(&comment).cloned()
    }

    fn break_taken(&self, tag: BreakTag, broken: bool) -> State {
        let currently = self.break_tags_taken.contains(&tag);
        if currently != broken {
            let mut s = self.clone();
            if broken {
                s.break_tags_taken.insert(tag);
            } else {
                s.break_tags_taken.remove(&tag);
            }
            return s;
        }
        self.clone()
    }

    fn with_indent_incremented_by(&self, plus: &Indent) -> State {
        let mut s = self.clone();
        s.indent = self.indent + plus.eval(self);
        s.must_break = false;
        s
    }

    fn with_no_indent(&self) -> State {
        let mut s = self.clone();
        s.indent = self.last_indent;
        s.must_break = false;
        s
    }

    fn with_broken_level(&self) -> State {
        let mut s = self.clone();
        s.last_indent = self.indent;
        s
    }

    fn with_break(&self, brk: DocId, b: &BreakDoc, broken: bool) -> State {
        let mut s = self.clone();
        if broken {
            let new_column = (self.indent + b.plus_indent.eval(self)).max(0);
            s.last_indent = self.indent;
            s.column = new_column;
            s.num_lines += 1;
            s.break_states.insert(brk, new_column);
        } else {
            s.column = self.column + b.flat.len() as i32;
        }
        s
    }

    fn update_after_level(&self, after: &State) -> State {
        State {
            last_indent: self.last_indent,
            indent: self.indent,
            branching_coefficient: self.branching_coefficient,
            must_break: self.must_break,
            column: after.column,
            num_lines: after.num_lines,
            break_tags_taken: after.break_tags_taken.clone(),
            break_states: after.break_states.clone(),
            level_states: after.level_states.clone(),
            tok_states: after.tok_states.clone(),
        }
    }

    fn add_new_lines(&self, n: i32) -> State {
        let mut s = self.clone();
        s.num_lines += n;
        s
    }

    fn with_column(&self, column: i32) -> State {
        let mut s = self.clone();
        s.column = column;
        s
    }

    fn with_must_break(&self, must_break: bool) -> State {
        let mut s = self.clone();
        s.must_break = must_break;
        s
    }

    fn with_new_branch(&self) -> State {
        let mut s = self.clone();
        s.branching_coefficient += 1;
        s
    }

    fn with_level_state(&self, level: DocId, one_line: bool) -> State {
        let mut s = self.clone();
        s.level_states.insert(level, one_line);
        s
    }

    fn with_tok_state(&self, comment: DocId, text: Rc<str>) -> State {
        let mut s = self.clone();
        s.tok_states.insert(comment, text);
        s
    }
}

// --------------------------------------------------------------------------------------------------------------
// Layout (port of Level.computeBreaks and friends)
// --------------------------------------------------------------------------------------------------------------

const LAST_LEVELS_TO_EXPLORE: i32 = 8;

pub struct Layout<'d, 'a> {
    pub docs: &'d Docs<'a>,
    pub comments: &'d CommentsHelper,
    pub max_width: i32,
}

impl Layout<'_, '_> {
    pub fn compute_breaks(&self, id: DocId, state: &State) -> State {
        let docs = self.docs;
        match &docs.docs[id].kind {
            DocKind::Level(_) => self.level_compute_breaks(id, state),
            DocKind::Token(t) => {
                let text = docs.token_text(t);
                if text.starts_with(TEXT_BLOCK_DELIMITER) {
                    state.with_column(state.column + TEXT_BLOCK_DELIMITER.len() as i32)
                } else {
                    state.with_column(state.column + jlen(text) as i32)
                }
            }
            DocKind::Comment(tok) => {
                let text = self.comments.rewrite(docs.input.tok(*tok), self.max_width, state.column);
                let offsets = newlines::line_offsets(&text);
                let first_line_length = jlen(&text[*offsets.last().unwrap()..]) as i32;
                state
                    .with_column(state.column + first_line_length)
                    .add_new_lines(offsets.len() as i32)
                    .with_tok_state(id, text.into())
            }
            DocKind::Space => state.with_column(state.column + 1),
            DocKind::Break(_) => panic!("Did you mean computeBreaks(State, int, boolean)?"),
        }
    }

    fn break_compute_breaks(&self, brk: DocId, state: &State, broken: bool) -> State {
        let b = self.docs.as_break(brk).unwrap();
        let state = match b.opt_tag {
            Some(tag) => state.break_taken(tag, broken),
            None => state.clone(),
        };
        state.with_break(brk, b, broken)
    }

    fn level_compute_breaks(&self, level: DocId, state: &State) -> State {
        let l = self.docs.level(level);
        if let Some(w) = self.try_to_fit_on_one_line(level, state.column, &l.docs) {
            return state.with_column(w).with_level_state(level, true);
        }
        let new_state = match l.open.break_behaviour {
            BreakBehaviour::BreakThisLevel => self.break_normally(level, state),
            BreakBehaviour::PreferBreakingLastInnerLevel { .. } => self.prefer_breaking_last_inner_level(level, state),
            BreakBehaviour::BreakOnlyIfInnerLevelsThenFitOnOneLine { keep_indent_when_inlined } => {
                let broken = self.break_normally(level, state);
                match self.handle_break_only_if_inner_levels_then_fit_on_one_line(
                    level,
                    state,
                    &broken,
                    keep_indent_when_inlined,
                ) {
                    Some(s) => s,
                    None => broken,
                }
            }
            BreakBehaviour::InlineSuffix => match self.try_inline_suffix(level, state, true) {
                Some(s) => s,
                None => self.break_normally(level, state),
            },
        };
        state.update_after_level(&new_state)
    }

    fn prefer_breaking_last_inner_level(&self, level: DocId, state: &State) -> State {
        let max_depth = self.docs.max_depth(level);
        if max_depth <= LAST_LEVELS_TO_EXPLORE {
            let state = state.with_new_branch();
            let broken = self.break_normally(level, &state);
            let state1 = state.with_no_indent();
            if let Some(last_level_broken) = self.try_break_last_level(level, &state1, true)
                && last_level_broken.num_lines < broken.num_lines
            {
                return last_level_broken;
            }
            broken
        } else {
            let state1 = state.with_no_indent();
            if let Some(s) = self.try_break_last_level(level, &state1, true) {
                return s;
            }
            self.break_normally(level, state)
        }
    }

    fn break_normally(&self, level: DocId, state: &State) -> State {
        let state_for_broken = state.with_indent_incremented_by(&self.docs.level(level).open.plus_indent);
        self.compute_broken(level, &state_for_broken)
    }

    /// Try to fit these docs belonging to `level` onto one line, returning the new column if possible.
    fn try_to_fit_on_one_line(&self, level: DocId, start_column: i32, docs: &[DocId]) -> Option<i32> {
        let mut column = start_column;
        let mut column_before_last_break = 0;
        for &doc in docs {
            if let Some(b) = self.docs.as_break(doc)
                && b.has_column_limit
            {
                column_before_last_break = column;
            } else if let Some(inner) = self.docs.as_level(doc) {
                column = self.try_to_fit_on_one_line(doc, column, &inner.docs)?;
                continue;
            }
            column = (column as f32 + self.docs.width(doc)) as i32;
        }
        if let Some(limit) = self.docs.level(level).open.column_limit_before_last_break
            && column_before_last_break > limit
        {
            return None;
        }
        if column <= self.max_width { Some(column) } else { None }
    }

    fn handle_break_only_if_inner_levels_then_fit_on_one_line(
        &self,
        level: DocId,
        state: &State,
        broken_state: &State,
        keep_indent: bool,
    ) -> Option<State> {
        let inner_levels = self.docs.non_empty_inner_levels(level);
        let any_level_was_broken = inner_levels.iter().any(|&l| !broken_state.is_one_line(l));
        if !any_level_was_broken {
            return Some(broken_state.clone());
        }
        let partially_inlined = self.try_inline_prefix_onto_current_line(level, state, keep_indent)?;
        let body_is_complex = inner_levels.iter().any(|&l| !self.docs.level(l).open.simple);
        if body_is_complex || partially_inlined.num_lines < broken_state.num_lines {
            return Some(partially_inlined);
        }
        None
    }

    fn try_inline_prefix_onto_current_line(&self, level: DocId, state: &State, keep_indent: bool) -> Option<State> {
        let last_level = *self
            .docs
            .non_empty_inner_levels(level)
            .last()
            .expect("Levels were broken so expected to find at least a non-empty level");
        let docs = &self.docs.level(level).docs;
        let idx = docs.iter().position(|&d| d == last_level).unwrap();
        let mut leading_width = self.docs.sum_width(&docs[..idx]);
        leading_width += self.docs.count_width_until_break(last_level, self.max_width - state.indent);
        let fits = !leading_width.is_infinite() && state.column as f32 + leading_width <= self.max_width as f32;
        if !fits {
            return None;
        }
        let mut new_state = state.with_no_indent();
        if keep_indent {
            new_state = new_state.with_indent_incremented_by(&self.docs.level(level).open.plus_indent);
        }
        Some(self.try_to_lay_out_level_on_one_line(&new_state, &self.docs.splits(level)))
    }

    fn try_break_last_level(&self, level: DocId, state: &State, is_simple_inlining_so_far: bool) -> Option<State> {
        let docs = &self.docs.level(level).docs;
        let &last = docs.last()?;
        if !self.docs.is_level(last) {
            return None;
        }
        self.try_break_inner_level(level, state, last, is_simple_inlining_so_far)
    }

    fn try_inline_suffix(&self, level: DocId, state: &State, is_simple_inlining_so_far: bool) -> Option<State> {
        let docs = &self.docs.level(level).docs;
        let last = *docs.last().expect("inlineSuffix level must end with a level");
        assert!(self.docs.is_level(last), "Level with break behaviour inlineSuffix must end with a level");
        assert!(
            self.docs.starts_with_break(last) == StartsWithBreak::Yes,
            "Level with break behaviour inlineSuffix must have a last level that starts with a break"
        );
        if docs.len() < 2 || !self.docs.is_level(docs[docs.len() - 2]) {
            return None;
        }
        self.try_break_inner_level(level, state, docs[docs.len() - 2], is_simple_inlining_so_far)
    }

    fn try_break_inner_level(
        &self,
        level: DocId,
        state: &State,
        inner_level: DocId,
        is_simple_inlining_so_far: bool,
    ) -> Option<State> {
        let inner = self.docs.level(inner_level);
        if inner.open.breakability_if_last_level == LastLevelBreakability::Abort {
            return None;
        }
        let docs = &self.docs.level(level).docs;
        let inner_index = docs.iter().position(|&d| d == inner_level).unwrap();
        let leading_docs = &docs[..inner_index];
        self.try_to_fit_on_one_line(level, state.column, leading_docs)?;
        let trailing_docs = &docs[inner_index + 1..];
        let trailing_width = self.docs.sum_width(trailing_docs);
        if trailing_width.is_infinite() {
            return None;
        }
        let prefix_splits = self.docs.split_by_breaks(leading_docs);
        let is_simple_inlining = is_simple_inlining_so_far && self.docs.level(level).open.simple;
        let state1 = self.try_to_lay_out_level_on_one_line(state, &prefix_splits);
        if state1.num_lines != state.num_lines {
            return None;
        }
        let state2 = match inner.open.breakability_if_last_level {
            LastLevelBreakability::AcceptInlineChainIfSimpleOtherwiseCheckInner if is_simple_inlining => {
                self.try_break_inner_level_accept_inline_chain(inner_level, &state1)
            }
            LastLevelBreakability::AcceptInlineChainIfSimpleOtherwiseCheckInner | LastLevelBreakability::CheckInner => {
                self.try_break_inner_level_check_inner(inner_level, is_simple_inlining, &state1)
            }
            LastLevelBreakability::AcceptInlineChain => {
                self.try_break_inner_level_accept_inline_chain(inner_level, &state1)
            }
            LastLevelBreakability::Abort => None,
        }?;
        let column = self.try_to_fit_on_one_line(level, state2.column, trailing_docs)?;
        Some(state2.with_column(column))
    }

    fn try_break_inner_level_accept_inline_chain(&self, last_level: DocId, state: &State) -> Option<State> {
        let extra_width = self.docs.count_width_until_break(last_level, self.max_width - state.indent);
        let still_fits = !extra_width.is_infinite() && state.column as f32 + extra_width <= self.max_width as f32;
        if !still_fits {
            return None;
        }
        Some(self.compute_breaks(last_level, state))
    }

    fn try_break_inner_level_check_inner(
        &self,
        inner_level: DocId,
        is_simple_inlining: bool,
        state: &State,
    ) -> Option<State> {
        let inner = self.docs.level(inner_level);
        match inner.open.break_behaviour {
            BreakBehaviour::PreferBreakingLastInnerLevel { keep_indent_when_inlined } => {
                let state1 = if keep_indent_when_inlined {
                    state.with_indent_incremented_by(&inner.open.plus_indent)
                } else {
                    state.clone()
                };
                self.try_break_last_level(inner_level, &state1, is_simple_inlining)
            }
            BreakBehaviour::InlineSuffix => self.try_inline_suffix(inner_level, state, is_simple_inlining),
            BreakBehaviour::BreakOnlyIfInnerLevelsThenFitOnOneLine { keep_indent_when_inlined } => {
                let state1 = if keep_indent_when_inlined {
                    state.with_indent_incremented_by(&inner.open.plus_indent)
                } else {
                    state.clone()
                };
                let &last2 = inner.docs.last()?;
                let last_level2 = self.docs.as_level(last2)?;
                match last_level2.open.breakability_if_last_level {
                    LastLevelBreakability::AcceptInlineChain => {
                        let broken = self.break_normally(inner_level, state);
                        self.handle_break_only_if_inner_levels_then_fit_on_one_line(
                            inner_level,
                            &state1,
                            &broken,
                            keep_indent_when_inlined,
                        )
                    }
                    _ => None,
                }
            }
            BreakBehaviour::BreakThisLevel => None,
        }
    }

    fn try_to_lay_out_level_on_one_line(&self, state: &State, sb: &SplitsBreaks) -> State {
        let mut state = state.clone();
        for i in 0..sb.splits.len() {
            if i > 0 {
                state = self.break_compute_breaks(sb.breaks[i - 1], &state, false);
            }
            let split = &sb.splits[i];
            let split_width = self.docs.sum_width(split);
            let enough_room = state.column as f32 + split_width <= self.max_width as f32;
            state = self.compute_split(split, &state.with_must_break(false));
            if !enough_room {
                state = state.with_must_break(true);
            }
        }
        state
    }

    fn compute_broken(&self, level: DocId, state: &State) -> State {
        let sb = self.docs.splits(level);
        let mut state = state.clone();
        if !sb.breaks.is_empty() {
            state = state.with_broken_level();
        }
        state = self.compute_break_and_split(level, &state, None, &sb.splits[0]);
        for i in 0..sb.breaks.len() {
            state = self.compute_break_and_split(level, &state, Some(sb.breaks[i]), &sb.splits[i + 1]);
        }
        state
    }

    fn compute_break_and_split(&self, level: DocId, state: &State, brk: Option<DocId>, split: &[DocId]) -> State {
        let break_width = brk.map(|b| self.docs.width(b)).unwrap_or(0.0);
        let split_width = self.docs.sum_width(split);
        let should_break = brk.is_some_and(|b| self.docs.as_break(b).unwrap().fill_mode == FillMode::Unified)
            || state.must_break
            || break_width.is_infinite()
            || self.try_to_fit_on_one_line(level, (state.column as f32 + break_width) as i32, split).is_none();
        let mut state = state.clone();
        if let Some(b) = brk {
            state = self.break_compute_breaks(b, &state, should_break);
        }
        let enough_room = state.column as f32 + split_width <= self.max_width as f32;
        state = self.compute_split(split, &state.with_must_break(false));
        if !enough_room {
            state = state.with_must_break(true);
        }
        state
    }

    fn compute_split(&self, docs: &[DocId], state: &State) -> State {
        let mut state = state.clone();
        for &d in docs {
            state = self.compute_breaks(d, &state);
        }
        state
    }
}
