//! A hand-written recursive-descent Java parser that builds the AST of [`crate::ast`] directly.
//!
//! It produces exactly the trees that converting tree-sitter's syntax tree produces (`tests/support/converter.rs`),
//! including the converter's positions and its adjustments to tree-sitter's shapes, and is checked against it. It
//! does not recover from errors: it reports where parsing stopped.

use crate::ast::{
    B, Case, CaseKind, Catch, ClassDecl, ClassKind, EnumConstant, K, LitKind, MethodDecl, Mods, Tree, VarDecl,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::lexer::{RawKind, lex};

/// Where parsing stopped (a byte offset), for diagnostics.
#[derive(Debug, Clone, Copy)]
pub struct ParseError {
    pub offset: usize,
}

type P<T> = Result<T, ParseError>;

thread_local! {
    static LAST: RefCell<Option<(String, P<Rc<Tree>>)>> = const { RefCell::new(None) };
}

/// [`parse`], reusing the result if `text` is the text of the previous call on this thread: formatting and the
/// analysis of unused imports parse the same input.
pub fn parse_cached(text: &str) -> P<Rc<Tree>> {
    LAST.with_borrow_mut(|last| {
        if let Some((t, tree)) = last.as_ref()
            && t == text
        {
            return tree.clone();
        }
        let tree = parse(text).map(Rc::new);
        *last = Some((text.to_string(), tree.clone()));
        tree
    })
}

/// Parses a compilation unit.
pub fn parse(src: &str) -> P<Tree> {
    let (raw, _) = lex(src, false).map_err(|_| ParseError { offset: 0 })?;
    let mut toks = Vec::with_capacity(raw.len() / 2 + 1);
    let mut first_comment = None;
    for (kind, start, end) in raw {
        match kind {
            RawKind::Whitespace => {}
            RawKind::Comment => {
                first_comment.get_or_insert(start);
            }
            // `>>`, `>=` etc. are split, so that type arguments can close one `>` at a time; expressions join them.
            RawKind::Token if src.as_bytes()[start] == b'>' && end - start > 1 => {
                toks.extend((start..end).map(|i| (i, i + 1)));
            }
            RawKind::Token => toks.push((start, end)),
        }
    }
    let mut p = Parser { src, toks, pos: 0, prev_end: 0, ctor_body: false, in_label: false };
    let unit = p.compilation_unit(first_comment)?;
    if p.pos < p.toks.len() {
        return p.fail();
    }
    Ok(unit)
}

#[rustfmt::skip]
const RESERVED: &[&str] = &[
    "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const", "continue",
    "default", "do", "double", "else", "enum", "extends", "final", "finally", "float", "for", "goto", "if",
    "implements", "import", "instanceof", "int", "interface", "long", "native", "new", "package", "private",
    "protected", "public", "return", "short", "static", "strictfp", "super", "switch", "synchronized", "this",
    "throw", "throws", "transient", "try", "void", "volatile", "while", "true", "false", "null",
];

#[rustfmt::skip]
const MODIFIERS: &[&str] = &[
    "public", "protected", "private", "static", "abstract", "final", "native", "synchronized", "transient",
    "volatile", "strictfp", "default", "sealed",
];

fn is_ident(t: &str) -> bool {
    t.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$') && !RESERVED.contains(&t)
}

fn is_primitive(t: &str) -> bool {
    matches!(t, "int" | "long" | "short" | "byte" | "char" | "boolean" | "float" | "double" | "void")
}

fn is_literal(t: &str) -> bool {
    let b = t.as_bytes();
    match b.first() {
        Some(c) if c.is_ascii_digit() => true,
        Some(b'.') => b.get(1).is_some_and(u8::is_ascii_digit),
        Some(b'"' | b'\'') => true,
        _ => matches!(t, "true" | "false" | "null"),
    }
}

fn literal_kind(t: &str) -> LitKind {
    match t.as_bytes()[0] {
        b'"' => return LitKind::String,
        b'\'' => return LitKind::Char,
        _ => {}
    }
    match t {
        "true" | "false" => return LitKind::Boolean,
        "null" => return LitKind::Null,
        _ => {}
    }
    let lower = t.to_ascii_lowercase();
    let float = if lower.starts_with("0x") {
        lower.contains('p')
    } else if lower.starts_with("0b") {
        false
    } else {
        lower.contains(['.', 'e']) || lower.ends_with(['f', 'd'])
    };
    match (float, lower.as_bytes()[lower.len() - 1]) {
        (true, b'f') => LitKind::Float,
        (true, _) => LitKind::Double,
        (false, b'l') => LitKind::Long,
        (false, _) => LitKind::Int,
    }
}

/// A decimal (not octal, hex or binary) integer literal, which tree-sitter folds a preceding `-` into.
fn is_decimal_integer(t: &str) -> bool {
    let b = t.as_bytes();
    let starts_decimal = match b.first() {
        Some(b'1'..=b'9') => true,
        Some(b'0') => matches!(&b[1..], [] | [b'l' | b'L']),
        _ => false,
    };
    starts_decimal && !t.contains(['.', 'e', 'E', 'x', 'X']) && !t.ends_with(['f', 'F', 'd', 'D'])
}

fn binary_precedence(op: &str) -> Option<i32> {
    Some(match op {
        "||" => 4,
        "&&" => 5,
        "|" => 6,
        "^" => 7,
        "&" => 8,
        "==" | "!=" => 9,
        "<" | ">" | "<=" | ">=" | "instanceof" => 10,
        "<<" | ">>" | ">>>" => 11,
        "+" | "-" => 12,
        "*" | "/" | "%" => 13,
        _ => return None,
    })
}

const ASSIGN_OPS: &[&str] = &["=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=", ">>>="];

/// A parsed `case`/`default` label of a switch, before the labels of a case are combined.
struct Label {
    /// The `default` keyword's range, if the label has one.
    default: Option<(usize, usize)>,
    /// Whether the label is a lone `default`.
    default_only: bool,
    items: Vec<Tree>,
    guard: Option<Tree>,
}

struct Parser<'s> {
    src: &'s str,
    toks: Vec<(usize, usize)>,
    pos: usize,
    /// The end of the last consumed token.
    prev_end: usize,
    /// Whether statements are parsed directly in a constructor body (for explicit constructor invocations).
    ctor_body: bool,
    /// Whether a case label is parsed, where `x ->` ends the label rather than starting a lambda.
    in_label: bool,
}

impl<'s> Parser<'s> {
    // ---- Tokens ----

    fn fail<T>(&self) -> P<T> {
        Err(ParseError { offset: self.cur_start() })
    }

    fn text_at(&self, i: usize) -> &'s str {
        match self.toks.get(i) {
            Some(&(s, e)) => &self.src[s..e],
            None => "",
        }
    }

    fn peek(&self) -> &'s str {
        self.text_at(self.pos)
    }

    fn peek_n(&self, n: usize) -> &'s str {
        self.text_at(self.pos + n)
    }

    fn cur_start(&self) -> usize {
        self.toks.get(self.pos).map_or(self.src.len(), |t| t.0)
    }

    fn cur_end(&self) -> usize {
        self.toks.get(self.pos).map_or(self.src.len(), |t| t.1)
    }

    /// Whether token `i` ends where token `i + 1` starts.
    fn adjacent(&self, i: usize) -> bool {
        matches!((self.toks.get(i), self.toks.get(i + 1)), (Some(a), Some(b)) if a.1 == b.0)
    }

    fn bump(&mut self) -> P<&'s str> {
        match self.toks.get(self.pos) {
            Some(&(s, e)) => {
                self.pos += 1;
                self.prev_end = e;
                Ok(&self.src[s..e])
            }
            None => self.fail(),
        }
    }

    fn bump_n(&mut self, n: usize) -> P<()> {
        for _ in 0..n {
            self.bump()?;
        }
        Ok(())
    }

    fn eat(&mut self, t: &str) -> bool {
        if self.peek() == t {
            self.pos += 1;
            self.prev_end = self.toks[self.pos - 1].1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: &str) -> P<()> {
        if self.eat(t) { Ok(()) } else { self.fail() }
    }

    fn ident(&mut self) -> P<String> {
        if is_ident(self.peek()) { Ok(self.bump()?.to_string()) } else { self.fail() }
    }

    /// The start of the last consumed token.
    fn prev_start(&self) -> usize {
        self.toks[self.pos - 1].0
    }

    fn tree(&self, start: usize, kind: K) -> Tree {
        Tree::new(start, self.prev_end, kind)
    }

    /// An operator starting at the current token: `>`s are joined (with a following `=`) if adjacent. Returns the
    /// operator and its number of tokens.
    fn operator(&self) -> (&'s str, usize) {
        let t = self.peek();
        if t != ">" {
            return (t, 1);
        }
        let mut n = 1;
        while n < 3 && self.peek_n(n) == ">" && self.adjacent(self.pos + n - 1) {
            n += 1;
        }
        if self.peek_n(n) == "=" && self.adjacent(self.pos + n - 1) {
            n += 1;
        }
        let (s, _) = self.toks[self.pos];
        let e = self.toks[self.pos + n - 1].1;
        (&self.src[s..e], n)
    }

    // ---- Scanning ahead (without building trees) ----

    fn at_annotation(&self) -> bool {
        self.peek() == "@" && self.peek_n(1) != "interface"
    }

    /// Skips annotations from token `i`.
    fn scan_annotations(&self, mut i: usize) -> Option<usize> {
        while self.text_at(i) == "@" && self.text_at(i + 1) != "interface" {
            i += 1;
            if !is_ident(self.text_at(i)) {
                return None;
            }
            i += 1;
            while self.text_at(i) == "." && is_ident(self.text_at(i + 1)) {
                i += 2;
            }
            if self.text_at(i) == "(" {
                i = self.scan_balanced(i)?;
            }
        }
        Some(i)
    }

    /// From an opening `(`, the index after its matching `)`.
    fn scan_balanced(&self, mut i: usize) -> Option<usize> {
        let mut depth = 0;
        loop {
            match self.text_at(i) {
                "(" => depth += 1,
                ")" => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i + 1);
                    }
                }
                "" => return None,
                _ => {}
            }
            i += 1;
        }
    }

    /// Skips a type (with annotations, type arguments and dimensions) from token `i`.
    fn scan_type(&self, i: usize) -> Option<usize> {
        let mut i = self.scan_annotations(i)?;
        let t = self.text_at(i);
        if is_primitive(t) {
            i += 1;
        } else {
            if !is_ident(t) {
                return None;
            }
            i += 1;
            loop {
                if self.text_at(i) == "<" {
                    i = self.scan_type_args(i)?;
                }
                if self.text_at(i) == "." && (is_ident(self.text_at(i + 1)) || self.text_at(i + 1) == "@") {
                    i = self.scan_annotations(i + 1)?;
                    if !is_ident(self.text_at(i)) {
                        return None;
                    }
                    i += 1;
                    continue;
                }
                break;
            }
        }
        loop {
            let j = self.scan_annotations(i)?;
            if self.text_at(j) == "[" && self.text_at(j + 1) == "]" {
                i = j + 2;
            } else {
                return Some(i);
            }
        }
    }

    fn scan_type_args(&self, mut i: usize) -> Option<usize> {
        i += 1;
        if self.text_at(i) == ">" {
            return Some(i + 1);
        }
        loop {
            let mut j = self.scan_annotations(i)?;
            if self.text_at(j) == "?" {
                j += 1;
                if matches!(self.text_at(j), "extends" | "super") {
                    j = self.scan_type(j + 1)?;
                }
            } else {
                j = self.scan_type(j)?;
            }
            i = j;
            match self.text_at(i) {
                "," => i += 1,
                ">" => return Some(i + 1),
                _ => return None,
            }
        }
    }

    /// Skips modifiers (keywords and annotations).
    fn scan_modifiers(&self, mut i: usize) -> Option<usize> {
        loop {
            let j = self.scan_annotations(i)?;
            if j > i {
                i = j;
                continue;
            }
            if MODIFIERS.contains(&self.text_at(i)) {
                i += 1;
                continue;
            }
            return Some(i);
        }
    }

    /// Whether a local variable declaration starts at the current token (without modifiers).
    fn at_local_var_decl(&self) -> bool {
        let t = self.peek();
        if !(is_ident(t) || is_primitive(t)) {
            return false;
        }
        match self.scan_type(self.pos) {
            Some(j) => is_ident(self.text_at(j)) && matches!(self.text_at(j + 1), "=" | ";" | "," | "[" | ":" | "@"),
            None => false,
        }
    }

    /// Whether a lambda expression starts at the current token.
    fn at_lambda(&self) -> bool {
        if self.in_label {
            return false;
        }
        let t = self.peek();
        if is_ident(t) {
            return self.peek_n(1) == "->";
        }
        if t != "(" {
            return false;
        }
        // Only tokens that can appear in lambda parameters are skipped, so this stays cheap on other parentheses.
        let mut i = self.pos + 1;
        let mut depth = 0;
        loop {
            let t = self.text_at(i);
            match t {
                ")" => return self.text_at(i + 1) == "->",
                "@" => {
                    i = match self.scan_annotations(i) {
                        Some(j) => j,
                        None => return false,
                    };
                    continue;
                }
                "<" => depth += 1,
                ">" => depth -= 1,
                "," | "." | "[" | "]" | "?" | "&" | "..." | "final" | "extends" | "super" => {}
                _ if is_ident(t) || is_primitive(t) => {}
                _ => return false,
            }
            if depth < 0 {
                return false;
            }
            i += 1;
        }
    }

    // ---- Compilation unit ----

    /// `first_comment` is the start of the first comment, where the tree of input without tokens starts.
    fn compilation_unit(&mut self, first_comment: Option<usize>) -> P<Tree> {
        let start = self.cur_start();
        let mut package = None;
        let mut imports = Vec::new();
        let mut types = Vec::new();
        let mut module = None;
        while self.pos < self.toks.len() {
            let after_annotations = self.scan_annotations(self.pos).unwrap_or(self.pos);
            match self.text_at(after_annotations) {
                "package" => {
                    let mut annotations = Vec::new();
                    while self.at_annotation() {
                        annotations.push(self.annotation()?);
                    }
                    self.expect("package")?;
                    let name = self.qualified_name()?;
                    self.expect(";")?;
                    package = Some((annotations, Box::new(name)));
                }
                "import" if after_annotations == self.pos => imports.push(self.import()?),
                ";" if after_annotations == self.pos => {
                    self.bump()?;
                }
                "module" => module = Some(Box::new(self.module()?)),
                "open" if self.text_at(after_annotations + 1) == "module" => module = Some(Box::new(self.module()?)),
                _ => {
                    let (start, pos) = (self.cur_start(), self.pos);
                    let mods = self.modifiers()?;
                    if self.at_class_decl() {
                        types.push(self.class_decl(start, mods)?);
                    } else {
                        // A field or method of the implicit class of a compact source file (Java 25).
                        self.pos = pos;
                        types.push(self.member(ClassKind::Class)?);
                    }
                }
            }
        }
        // tree-sitter's root node extends to the end of the input.
        let start = if self.toks.is_empty() { first_comment.unwrap_or(self.src.len()) } else { start };
        Ok(Tree::new(start, self.src.len(), K::CompilationUnit { package, imports, types, module }))
    }

    fn import(&mut self) -> P<Tree> {
        let start = self.cur_start();
        self.expect("import")?;
        let is_static = self.eat("static");
        // A module import (Java 25), `import module java.base;`, is kept as an import of the module's name.
        if !is_static && self.peek() == "module" && is_ident(self.peek_n(1)) {
            self.bump()?;
        }
        let mut name = self.qualified_name()?;
        if self.peek() == "." && self.peek_n(1) == "*" {
            self.bump_n(2)?;
            name = Tree::new(name.start, self.prev_end, K::MemberSelect { expr: Box::new(name), name: "*".into() });
        }
        self.expect(";")?;
        Ok(self.tree(start, K::Import { is_static, name: Box::new(name) }))
    }

    fn module(&mut self) -> P<Tree> {
        let start = self.cur_start();
        let mut annotations = Vec::new();
        while self.at_annotation() {
            annotations.push(self.annotation()?);
        }
        let open = self.eat("open");
        self.expect("module")?;
        let name = Box::new(self.qualified_name()?);
        self.expect("{")?;
        let mut directives = Vec::new();
        while !self.eat("}") {
            let d = self.cur_start();
            let kind = match self.bump()? {
                "requires" => {
                    while matches!(self.peek(), "transitive" | "static") && !matches!(self.peek_n(1), ";" | ".") {
                        self.bump()?;
                    }
                    K::Requires { name: Box::new(self.qualified_name()?) }
                }
                "uses" => K::Uses { name: Box::new(self.qualified_name()?) },
                keyword @ ("exports" | "opens" | "provides") => {
                    let name = Box::new(self.qualified_name()?);
                    let separator = if keyword == "provides" { "with" } else { "to" };
                    let mut items = Vec::new();
                    if self.eat(separator) {
                        loop {
                            items.push(self.qualified_name()?);
                            if !self.eat(",") {
                                break;
                            }
                        }
                    }
                    let keyword = match keyword {
                        "exports" => "exports",
                        "opens" => "opens",
                        _ => "provides",
                    };
                    K::Directive { keyword, name, separator, items: if items.is_empty() { None } else { Some(items) } }
                }
                _ => return self.fail(),
            };
            self.expect(";")?;
            directives.push(self.tree(d, kind));
        }
        Ok(self.tree(start, K::Module { annotations, open, name, directives }))
    }

    /// A (possibly qualified) name, as a chain of `MemberSelect`s.
    fn qualified_name(&mut self) -> P<Tree> {
        let start = self.cur_start();
        let mut name = Tree::new(start, self.cur_end(), K::Ident(self.ident()?));
        while self.peek() == "." && is_ident(self.peek_n(1)) {
            self.bump()?;
            let part = self.ident()?;
            name = self.tree(start, K::MemberSelect { expr: Box::new(name), name: part });
        }
        Ok(name)
    }

    // ---- Annotations and modifiers ----

    fn annotation(&mut self) -> P<Tree> {
        let start = self.cur_start();
        self.expect("@")?;
        let ty = Box::new(self.qualified_name()?);
        let mut args = Vec::new();
        if self.eat("(") && !self.eat(")") {
            if is_ident(self.peek()) && self.peek_n(1) == "=" {
                loop {
                    let key_start = self.cur_start();
                    let key = Tree::new(key_start, self.cur_end(), K::Ident(self.ident()?));
                    self.expect("=")?;
                    let value = self.element_value()?;
                    args.push(
                        self.tree(key_start, K::Assign { op: "=".into(), lhs: Box::new(key), rhs: Box::new(value) }),
                    );
                    if !self.eat(",") {
                        break;
                    }
                }
            } else {
                args.push(self.element_value()?);
            }
            self.expect(")")?;
        }
        Ok(self.tree(start, K::Annotation { ty, args }))
    }

    fn element_value(&mut self) -> P<Tree> {
        if self.at_annotation() {
            return self.annotation();
        }
        if self.peek() == "{" {
            let start = self.cur_start();
            self.bump()?;
            let mut init = Vec::new();
            while !self.eat("}") {
                init.push(self.element_value()?);
                if !self.eat(",") {
                    self.expect("}")?;
                    break;
                }
            }
            return Ok(
                self.tree(start, K::NewArray { elem: None, dims: vec![], dim_annotations: vec![], init: Some(init) })
            );
        }
        self.ternary()
    }

    fn modifiers(&mut self) -> P<Mods> {
        let start = self.cur_start();
        let mut annotations = Vec::new();
        loop {
            if self.at_annotation() {
                annotations.push(self.annotation()?);
                continue;
            }
            let t = self.peek();
            if t == "non" && self.peek_n(1) == "-" && self.peek_n(2) == "sealed" {
                self.bump_n(3)?;
                continue;
            }
            if MODIFIERS.contains(&t) {
                let next = self.peek_n(1);
                if t == "default" && matches!(next, ":" | "->" | ",") {
                    break;
                }
                if t == "sealed"
                    && !(is_ident(next) || MODIFIERS.contains(&next) || matches!(next, "class" | "interface" | "@"))
                {
                    break;
                }
                self.bump()?;
                continue;
            }
            break;
        }
        Ok(Mods { start, annotations })
    }

    // ---- Declarations ----

    fn at_class_decl(&self) -> bool {
        match self.peek() {
            "class" | "interface" | "enum" => true,
            "@" => self.peek_n(1) == "interface",
            "record" => is_ident(self.peek_n(1)) && matches!(self.peek_n(2), "(" | "<"),
            _ => false,
        }
    }

    fn class_decl(&mut self, start: usize, mods: Mods) -> P<Tree> {
        let kind = match self.bump()? {
            "class" => ClassKind::Class,
            "interface" => ClassKind::Interface,
            "enum" => ClassKind::Enum,
            "record" => ClassKind::Record,
            "@" => {
                self.expect("interface")?;
                ClassKind::Annotation
            }
            _ => return self.fail(),
        };
        let name = self.ident()?;
        let name_pos = self.prev_start();
        let type_params = if self.peek() == "<" { self.type_params()? } else { vec![] };
        let mut record_components = Vec::new();
        if kind == ClassKind::Record {
            self.expect("(")?;
            while !self.eat(")") {
                let mut v = self.formal_parameter()?;
                v.is_record_component = true;
                record_components.push(v);
                if !self.eat(",") {
                    self.expect(")")?;
                    break;
                }
            }
        }
        let mut extends = None;
        let mut implements = Vec::new();
        let mut permits = Vec::new();
        loop {
            match self.peek() {
                "extends" if kind == ClassKind::Interface => {
                    self.bump()?;
                    implements = self.type_list()?;
                }
                "extends" if kind == ClassKind::Class => {
                    self.bump()?;
                    extends = Some(Box::new(self.ty()?));
                }
                "implements" if kind != ClassKind::Interface && kind != ClassKind::Annotation => {
                    self.bump()?;
                    implements = self.type_list()?;
                }
                "permits" => {
                    self.bump()?;
                    permits = self.type_list()?;
                }
                _ => break,
            }
        }
        let mut members = Vec::new();
        let mut enum_constants = Vec::new();
        self.expect("{")?;
        if kind == ClassKind::Enum {
            // An enum without constants may still have the separating comma: `enum E { , }`.
            if self.peek() == "," {
                self.bump()?;
            }
            while !matches!(self.peek(), ";" | "}") {
                enum_constants.push(self.enum_constant()?);
                if !self.eat(",") {
                    break;
                }
            }
            if self.eat(";") {
                self.members_until_brace(kind, &mut members)?;
            } else {
                self.expect("}")?;
            }
        } else {
            self.members_until_brace(kind, &mut members)?;
        }
        Ok(self.tree(
            start,
            K::Class(Box::new(ClassDecl {
                kind,
                mods,
                name,
                type_params,
                extends,
                implements,
                permits,
                record_components,
                enum_constants,
                members,
                name_pos,
            })),
        ))
    }

    fn type_list(&mut self) -> P<Vec<Tree>> {
        let mut out = vec![self.ty()?];
        while self.eat(",") {
            out.push(self.ty()?);
        }
        Ok(out)
    }

    fn enum_constant(&mut self) -> P<EnumConstant> {
        let mods = self.modifiers()?;
        let name = self.ident()?;
        let name_pos = self.prev_start();
        let args = if self.peek() == "(" { self.arguments()? } else { vec![] };
        let body = if self.peek() == "{" { Some(self.class_body(ClassKind::Class)?) } else { None };
        Ok(EnumConstant { mods, name, args, body, name_pos })
    }

    /// A class body, from `{`.
    fn class_body(&mut self, kind: ClassKind) -> P<Vec<Tree>> {
        self.expect("{")?;
        let mut members = Vec::new();
        self.members_until_brace(kind, &mut members)?;
        Ok(members)
    }

    fn members_until_brace(&mut self, kind: ClassKind, members: &mut Vec<Tree>) -> P<()> {
        while !self.eat("}") {
            if self.pos >= self.toks.len() {
                return self.fail();
            }
            if self.eat(";") {
                continue;
            }
            members.push(self.member(kind)?);
        }
        Ok(())
    }

    fn member(&mut self, kind: ClassKind) -> P<Tree> {
        let start = self.cur_start();
        if self.peek() == "{" {
            return self.block();
        }
        if self.peek() == "static" && self.peek_n(1) == "{" {
            self.bump()?;
            let mut b = self.block()?;
            if let K::Block { is_static, .. } = &mut b.kind {
                *is_static = true;
            }
            b.start = start;
            return Ok(b);
        }
        let mods = self.modifiers()?;
        if self.at_class_decl() {
            return self.class_decl(start, mods);
        }
        let mut type_params = Vec::new();
        let mut return_type_annotations = Vec::new();
        if self.peek() == "<" {
            type_params = self.type_params()?;
            while self.at_annotation() {
                return_type_annotations.push(self.annotation()?);
            }
        }
        if is_ident(self.peek()) && self.peek_n(1) == "(" {
            return self.method_rest(start, mods, return_type_annotations, type_params, None);
        }
        if kind == ClassKind::Record && is_ident(self.peek()) && self.peek_n(1) == "{" {
            let name = self.ident()?;
            let name_pos = self.prev_start();
            let body = self.ctor_block()?;
            return Ok(self.tree(
                start,
                K::Method(Box::new(MethodDecl {
                    mods,
                    return_type_annotations,
                    type_params,
                    ret: None,
                    name,
                    receiver: None,
                    params: vec![],
                    throws: vec![],
                    body: Some(Box::new(body)),
                    default_value: None,
                    compact_ctor: true,
                    name_pos,
                })),
            ));
        }
        let ty = self.ty()?;
        if is_ident(self.peek()) && self.peek_n(1) == "(" {
            return self.method_rest(start, mods, return_type_annotations, type_params, Some(ty));
        }
        let vars = self.var_declarators(start, &mods, Some(ty))?;
        self.expect(";")?;
        Ok(self.tree(start, K::Vars(vars)))
    }

    fn method_rest(
        &mut self,
        start: usize,
        mods: Mods,
        return_type_annotations: Vec<Tree>,
        type_params: Vec<Tree>,
        mut ret: Option<Tree>,
    ) -> P<Tree> {
        let is_ctor = ret.is_none();
        let name = self.ident()?;
        let name_pos = self.prev_start();
        let (receiver, params) = self.formal_parameters()?;
        if let Some(groups) = self.dims()? {
            ret = ret.map(|r| self.add_dims(r, groups));
        }
        let mut throws = Vec::new();
        if self.eat("throws") {
            throws = self.type_list()?;
        }
        let mut body = None;
        let mut default_value = None;
        if self.peek() == "{" {
            body = Some(Box::new(if is_ctor { self.ctor_block()? } else { self.block()? }));
        } else {
            if self.eat("default") {
                default_value = Some(Box::new(self.element_value()?));
            }
            self.expect(";")?;
        }
        // The converter takes every annotation child of the method as a return type annotation, including a default
        // value that is an annotation.
        let mut return_type_annotations = return_type_annotations;
        if let Some(v) = &default_value
            && matches!(v.kind, K::Annotation { .. })
        {
            return_type_annotations.push((**v).clone());
        }
        Ok(self.tree(
            start,
            K::Method(Box::new(MethodDecl {
                mods,
                return_type_annotations,
                type_params,
                ret: ret.map(Box::new),
                name,
                receiver,
                params,
                throws,
                body,
                default_value,
                compact_ctor: false,
                name_pos,
            })),
        ))
    }

    fn ctor_block(&mut self) -> P<Tree> {
        let outer = std::mem::replace(&mut self.ctor_body, true);
        let block = self.block();
        self.ctor_body = outer;
        block
    }

    fn type_params(&mut self) -> P<Vec<Tree>> {
        self.expect("<")?;
        let mut out = Vec::new();
        loop {
            let start = self.cur_start();
            let mut annotations = Vec::new();
            while self.at_annotation() {
                annotations.push(self.annotation()?);
            }
            let name = self.ident()?;
            let mut bounds = Vec::new();
            if self.eat("extends") {
                bounds.push(self.ty()?);
                while self.eat("&") {
                    bounds.push(self.ty()?);
                }
            }
            out.push(self.tree(start, K::TypeParameter { annotations, name, bounds }));
            if !self.eat(",") {
                break;
            }
        }
        self.expect(">")?;
        Ok(out)
    }

    fn formal_parameters(&mut self) -> P<(Option<Box<VarDecl>>, Vec<VarDecl>)> {
        self.expect("(")?;
        let mut receiver = None;
        let mut params = Vec::new();
        while !self.eat(")") {
            if self.at_receiver() {
                receiver = Some(Box::new(self.receiver_parameter()?));
            } else {
                params.push(self.formal_parameter()?);
            }
            if !self.eat(",") {
                self.expect(")")?;
                break;
            }
        }
        Ok((receiver, params))
    }

    /// tree-sitter parses `T this` and `@A T Outer.this` as receiver parameters, but `@A T this` as a formal
    /// parameter named `this`.
    fn at_receiver(&self) -> bool {
        let Some(j) = self.scan_type(self.pos) else { return false };
        (self.text_at(j) == "this" && self.peek() != "@")
            || (is_ident(self.text_at(j)) && self.text_at(j + 1) == "." && self.text_at(j + 2) == "this")
    }

    fn receiver_parameter(&mut self) -> P<VarDecl> {
        let start = self.cur_start();
        let mut annotations = Vec::new();
        while self.at_annotation() {
            annotations.push(self.annotation()?);
        }
        let ty = self.unannotated_type()?;
        let name_pos = self.cur_start();
        let mut parts = Vec::new();
        while self.peek() != "this" {
            parts.push(self.ident()?);
            self.expect(".")?;
        }
        self.bump()?;
        parts.push("this".to_string());
        Ok(VarDecl {
            start,
            end: self.prev_end,
            mods: Mods { start, annotations },
            ty: Some(Box::new(ty)),
            name: parts.join("."),
            init: None,
            is_record_component: false,
            name_pos,
        })
    }

    fn formal_parameter(&mut self) -> P<VarDecl> {
        let start = self.cur_start();
        let mods = self.modifiers()?;
        let mut ty = self.ty()?;
        if self.at_annotation() || self.peek() == "..." {
            let mut annotations = Vec::new();
            while self.at_annotation() {
                annotations.push(self.annotation()?);
            }
            self.expect("...")?;
            let name_start = self.cur_start();
            ty = Tree::new(ty.start, name_start, K::ArrayType { elem: Box::new(ty), dims: vec![annotations] });
        }
        let name = if self.peek() == "this" { self.bump()?.to_string() } else { self.ident()? };
        let name_pos = self.prev_start();
        let name_end = self.prev_end;
        if let Some(groups) = self.dims()? {
            ty = self.add_dims(ty, groups);
        }
        let end = self.prev_end.max(name_end);
        Ok(VarDecl { start, end, mods, ty: Some(Box::new(ty)), name, init: None, is_record_component: false, name_pos })
    }

    /// Variable declarators after the type, up to (not including) the `;`. `ty` is `None` for `var`.
    fn var_declarators(&mut self, start: usize, mods: &Mods, ty: Option<Tree>) -> P<Vec<VarDecl>> {
        let base = match ty {
            Some(t) if matches!(&t.kind, K::Ident(n) if n == "var") => None,
            other => other,
        };
        let mut vars = Vec::new();
        loop {
            let name = self.ident()?;
            let name_pos = self.prev_start();
            let mut ty = base.clone();
            if let Some(groups) = self.dims()? {
                ty = ty.map(|t| self.add_dims(t, groups));
            }
            let init = if self.eat("=") { Some(Box::new(self.var_init()?)) } else { None };
            vars.push(VarDecl {
                start,
                end: self.prev_end,
                mods: mods.clone(),
                ty: ty.map(Box::new),
                name,
                init,
                is_record_component: false,
                name_pos,
            });
            if !self.eat(",") {
                return Ok(vars);
            }
        }
    }

    fn var_init(&mut self) -> P<Tree> {
        if self.peek() == "{" { self.array_initializer() } else { self.expr() }
    }

    fn array_initializer(&mut self) -> P<Tree> {
        let start = self.cur_start();
        self.expect("{")?;
        let mut init = Vec::new();
        // `{,}` is an empty initializer.
        if self.peek() == "," && self.peek_n(1) == "}" {
            self.bump()?;
        }
        while !self.eat("}") {
            init.push(self.var_init()?);
            if !self.eat(",") {
                self.expect("}")?;
                break;
            }
        }
        Ok(self.tree(start, K::NewArray { elem: None, dims: vec![], dim_annotations: vec![], init: Some(init) }))
    }

    // ---- Types ----

    /// A type, with leading annotations.
    fn ty(&mut self) -> P<Tree> {
        let start = self.cur_start();
        let mut annotations = Vec::new();
        while self.at_annotation() {
            annotations.push(self.annotation()?);
        }
        let base = self.unannotated_type()?;
        Ok(self.annotate(start, annotations, base))
    }

    /// Wraps an annotated type like the converter: the annotations of an array type belong to its element type.
    fn annotate(&self, start: usize, annotations: Vec<Tree>, mut base: Tree) -> Tree {
        if annotations.is_empty() {
            return base;
        }
        let end = self.prev_end;
        if matches!(base.kind, K::ArrayType { .. })
            && let K::ArrayType { elem, dims } = base.take_kind()
        {
            let elem = Tree::new(start, elem.end, K::Annotated { annotations, base: elem });
            return Tree::new(start, end, K::ArrayType { elem: Box::new(elem), dims });
        }
        Tree::new(start, end, K::Annotated { annotations, base: Box::new(base) })
    }

    /// A primitive or class type with its dimensions, without leading annotations.
    fn unannotated_type(&mut self) -> P<Tree> {
        let base = self.element_type()?;
        Ok(match self.dims()? {
            Some(groups) => self.add_dims(base, groups),
            None => base,
        })
    }

    /// A primitive or class type, without dimensions.
    fn element_type(&mut self) -> P<Tree> {
        let start = self.cur_start();
        if is_primitive(self.peek()) {
            let name = self.bump()?.to_string();
            return Ok(self.tree(start, K::Primitive(name)));
        }
        let mut t = Tree::new(start, self.cur_end(), K::Ident(self.ident()?));
        loop {
            if self.peek() == "<" {
                let args = self.type_args()?;
                t = self.tree(start, K::Parameterized { base: Box::new(t), args });
            }
            if self.peek() == "."
                && (is_ident(self.peek_n(1)) || (self.peek_n(1) == "@" && self.peek_n(2) != "interface"))
            {
                self.bump()?;
                let mut annotations = Vec::new();
                while self.at_annotation() {
                    annotations.push(self.annotation()?);
                }
                let name = self.ident()?;
                let select = self.tree(start, K::MemberSelect { expr: Box::new(t), name });
                t = if annotations.is_empty() {
                    select
                } else {
                    self.tree(start, K::Annotated { annotations, base: Box::new(select) })
                };
                continue;
            }
            return Ok(t);
        }
    }

    fn type_args(&mut self) -> P<Vec<Tree>> {
        self.expect("<")?;
        let mut args = Vec::new();
        if self.eat(">") {
            return Ok(args);
        }
        loop {
            args.push(self.type_arg()?);
            if !self.eat(",") {
                break;
            }
        }
        self.expect(">")?;
        Ok(args)
    }

    fn type_arg(&mut self) -> P<Tree> {
        let start = self.cur_start();
        let mut annotations = Vec::new();
        while self.at_annotation() {
            annotations.push(self.annotation()?);
        }
        if self.eat("?") {
            let bound = match self.peek() {
                "extends" | "super" => {
                    let extends = self.bump()? == "extends";
                    Some((extends, Box::new(self.ty()?)))
                }
                _ => None,
            };
            let w = self.tree(start, K::Wildcard { bound });
            if annotations.is_empty() {
                return Ok(w);
            }
            return Ok(self.tree(start, K::Annotated { annotations, base: Box::new(w) }));
        }
        let base = self.unannotated_type()?;
        Ok(self.annotate(start, annotations, base))
    }

    /// Array dimensions (`[]`, each possibly annotated): the annotation groups, if there are any dimensions.
    fn dims(&mut self) -> P<Option<Vec<Vec<Tree>>>> {
        let mut groups = Vec::new();
        while let Some(j) = self.scan_annotations(self.pos) {
            if !(self.text_at(j) == "[" && self.text_at(j + 1) == "]") {
                break;
            }
            let mut annotations = Vec::new();
            while self.at_annotation() {
                annotations.push(self.annotation()?);
            }
            self.bump_n(2)?;
            groups.push(annotations);
        }
        Ok(if groups.is_empty() { None } else { Some(groups) })
    }

    /// Wraps `ty` with dimensions that end at the last consumed token.
    fn add_dims(&self, mut ty: Tree, groups: Vec<Vec<Tree>>) -> Tree {
        let (start, end) = (ty.start, self.prev_end);
        match ty.take_kind() {
            K::ArrayType { elem, dims: mut existing } => {
                existing.extend(groups);
                Tree::new(start, end, K::ArrayType { elem, dims: existing })
            }
            other => {
                let elem = Box::new(Tree::new(start, ty.end, other));
                Tree::new(start, end, K::ArrayType { elem, dims: groups })
            }
        }
    }

    // ---- Statements ----

    fn block(&mut self) -> P<Tree> {
        let start = self.cur_start();
        self.expect("{")?;
        let ctor = std::mem::replace(&mut self.ctor_body, false);
        let mut stmts = Vec::new();
        while !self.eat("}") {
            if self.pos >= self.toks.len() {
                return self.fail();
            }
            // Explicit constructor invocations may follow other statements (Java 22).
            stmts.push(self.block_stmt(ctor)?);
        }
        Ok(self.tree(start, K::Block { is_static: false, stmts }))
    }

    /// A statement or declaration in a block. `ctor_call` allows an explicit constructor invocation.
    fn block_stmt(&mut self, ctor_call: bool) -> P<Tree> {
        let start = self.cur_start();
        let t = self.peek();
        let declaration = match t {
            "final" | "abstract" | "static" | "strictfp" => true,
            "@" => true,
            "class" | "interface" | "enum" => true,
            "record" => self.at_class_decl(),
            "sealed" | "non" => {
                self.scan_modifiers(self.pos).is_some_and(|j| matches!(self.text_at(j), "class" | "interface"))
                    && t == "sealed"
            }
            _ => false,
        };
        if declaration {
            let mods = self.modifiers()?;
            if self.at_class_decl() {
                return self.class_decl(start, mods);
            }
            let ty = self.ty()?;
            let vars = self.var_declarators(start, &mods, Some(ty))?;
            self.expect(";")?;
            return Ok(self.tree(start, K::Vars(vars)));
        }
        if t != "yield" && self.at_local_var_decl() {
            let mods = Mods { start, annotations: vec![] };
            let ty = self.ty()?;
            let vars = self.var_declarators(start, &mods, Some(ty))?;
            self.expect(";")?;
            return Ok(self.tree(start, K::Vars(vars)));
        }
        self.stmt(ctor_call)
    }

    fn parens(&mut self) -> P<Tree> {
        let start = self.cur_start();
        self.expect("(")?;
        let e = self.expr()?;
        self.expect(")")?;
        Ok(self.tree(start, K::Parens(Box::new(e))))
    }

    fn stmt(&mut self, ctor_call: bool) -> P<Tree> {
        let start = self.cur_start();
        let kind = match self.peek() {
            "{" => return self.block(),
            ";" => {
                self.bump()?;
                K::Empty
            }
            "if" => {
                self.bump()?;
                let cond = Box::new(self.parens()?);
                let then = Box::new(self.stmt(false)?);
                let else_ = if self.eat("else") { Some(Box::new(self.stmt(false)?)) } else { None };
                K::If { cond, then, else_ }
            }
            "while" => {
                self.bump()?;
                let cond = Box::new(self.parens()?);
                K::While { cond, body: Box::new(self.stmt(false)?) }
            }
            "do" => {
                self.bump()?;
                let body = Box::new(self.stmt(false)?);
                self.expect("while")?;
                let cond = Box::new(self.parens()?);
                self.expect(";")?;
                K::DoWhile { body, cond }
            }
            "for" => return self.for_stmt(),
            "try" => return self.try_stmt(),
            "switch" => {
                let (selector, cases) = self.switch()?;
                K::Switch { selector, cases }
            }
            "return" => {
                self.bump()?;
                let e = if self.peek() == ";" { None } else { Some(Box::new(self.expr()?)) };
                self.expect(";")?;
                K::Return(e)
            }
            "throw" => {
                self.bump()?;
                let e = Box::new(self.expr()?);
                self.expect(";")?;
                K::Throw(e)
            }
            "break" | "continue" => {
                let keyword = self.bump()?;
                let label = if is_ident(self.peek()) { Some(self.ident()?) } else { None };
                self.expect(";")?;
                if keyword == "break" { K::Break(label) } else { K::Continue(label) }
            }
            "synchronized" => {
                self.bump()?;
                let lock = Box::new(self.parens()?);
                K::Synchronized { lock, block: Box::new(self.block()?) }
            }
            "assert" => {
                self.bump()?;
                let cond = Box::new(self.expr()?);
                let detail = if self.eat(":") { Some(Box::new(self.expr()?)) } else { None };
                self.expect(";")?;
                K::Assert { cond, detail }
            }
            "yield" if !matches!(self.peek_n(1), "=" | "." | "[" | "++" | "--" | "->") => {
                self.bump()?;
                let e = Box::new(self.expr()?);
                self.expect(";")?;
                K::Yield(e)
            }
            t if is_ident(t) && self.peek_n(1) == ":" => {
                let label = self.ident()?;
                self.bump()?;
                K::Labeled { label, body: Box::new(self.stmt(false)?) }
            }
            _ => {
                let e = self.expr()?;
                self.expect(";")?;
                if is_constructor_call(&e) {
                    // Explicit constructor invocations are only accepted directly in constructor bodies, like
                    // tree-sitter does; there, tree-sitter's invocation includes the `;`.
                    if !ctor_call {
                        return self.fail();
                    }
                    let mut e = e;
                    let e = Tree::new(e.start, self.prev_end, e.take_kind());
                    return Ok(self.tree(start, K::ExprStmt(Box::new(e))));
                }
                K::ExprStmt(Box::new(e))
            }
        };
        Ok(self.tree(start, kind))
    }

    fn for_stmt(&mut self) -> P<Tree> {
        let start = self.cur_start();
        self.expect("for")?;
        self.expect("(")?;
        if self.at_for_each() {
            // Without modifiers, the converter starts the variable (and its modifiers) at `for`.
            let mut mods = self.modifiers()?;
            if mods.start == self.cur_start() {
                mods.start = start;
            }
            let var_start = mods.start;
            let ty = self.ty()?;
            let name = self.ident()?;
            let name_pos = self.prev_start();
            let name_end = self.prev_end;
            let mut ty = if matches!(&ty.kind, K::Ident(n) if n == "var") { None } else { Some(ty) };
            if let Some(groups) = self.dims()? {
                ty = ty.map(|t| self.add_dims(t, groups));
            }
            self.expect(":")?;
            let var = VarDecl {
                start: var_start,
                end: name_end,
                mods,
                ty: ty.map(Box::new),
                name,
                init: None,
                is_record_component: false,
                name_pos,
            };
            let expr = Box::new(self.expr()?);
            self.expect(")")?;
            let body = Box::new(self.stmt(false)?);
            return Ok(self.tree(start, K::ForEach { var: Box::new(var), expr, body }));
        }
        let mut init = Vec::new();
        if !self.eat(";") {
            let decl_start = self.cur_start();
            let after_mods = self.scan_modifiers(self.pos).unwrap_or(self.pos);
            if after_mods > self.pos || self.at_local_var_decl() {
                let mods = self.modifiers()?;
                let ty = self.ty()?;
                let vars = self.var_declarators(decl_start, &mods, Some(ty))?;
                self.expect(";")?;
                init.push(self.tree(decl_start, K::Vars(vars)));
            } else {
                loop {
                    let e = self.expr()?;
                    init.push(Tree::new(e.start, e.end, K::ExprStmt(Box::new(e))));
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect(";")?;
            }
        }
        let cond = if self.peek() == ";" { None } else { Some(Box::new(self.expr()?)) };
        self.expect(";")?;
        let mut update = Vec::new();
        while !self.eat(")") {
            update.push(self.expr()?);
            if !self.eat(",") {
                self.expect(")")?;
                break;
            }
        }
        let body = Box::new(self.stmt(false)?);
        Ok(self.tree(start, K::For { init, cond, update, body }))
    }

    fn at_for_each(&self) -> bool {
        let Some(i) = self.scan_modifiers(self.pos) else { return false };
        let Some(mut j) = self.scan_type(i) else { return false };
        if !is_ident(self.text_at(j)) {
            return false;
        }
        j += 1;
        while let Some(k) = self.scan_annotations(j)
            && self.text_at(k) == "["
            && self.text_at(k + 1) == "]"
        {
            j = k + 2;
        }
        self.text_at(j) == ":"
    }

    fn try_stmt(&mut self) -> P<Tree> {
        let start = self.cur_start();
        self.expect("try")?;
        let mut resources = Vec::new();
        if self.eat("(") {
            while !self.eat(")") {
                resources.push(self.resource()?);
                if !self.eat(";") {
                    self.expect(")")?;
                    break;
                }
            }
        }
        let block = Box::new(self.block()?);
        let mut catches = Vec::new();
        while self.peek() == "catch" {
            let catch_start = self.cur_start();
            self.bump()?;
            self.expect("(")?;
            let param_start = self.cur_start();
            let mods = self.modifiers()?;
            let mut alternatives = vec![self.ty()?];
            while self.eat("|") {
                alternatives.push(self.ty()?);
            }
            let name = self.ident()?;
            let name_pos = self.prev_start();
            let param_end = self.prev_end;
            self.expect(")")?;
            let ty = if alternatives.len() == 1 {
                alternatives[0].clone()
            } else {
                let (s, e) = (alternatives[0].start, alternatives[alternatives.len() - 1].end);
                Tree::new(s, e, K::Union(alternatives.clone()))
            };
            let param = VarDecl {
                start: param_start,
                end: param_end,
                mods,
                ty: Some(Box::new(ty)),
                name,
                init: None,
                is_record_component: false,
                name_pos,
            };
            let union = if alternatives.len() > 1 { alternatives } else { vec![] };
            catches.push(Catch { start: catch_start, param, union, block: Box::new(self.block()?) });
        }
        let finally = if self.eat("finally") { Some(Box::new(self.block()?)) } else { None };
        Ok(self.tree(start, K::Try { resources, block, catches, finally }))
    }

    fn resource(&mut self) -> P<Tree> {
        let start = self.cur_start();
        let after_mods = self.scan_modifiers(self.pos).unwrap_or(self.pos);
        let is_decl = after_mods > self.pos
            || self.scan_type(self.pos).is_some_and(|j| is_ident(self.text_at(j)) && self.text_at(j + 1) == "=");
        if !is_decl {
            return self.expr();
        }
        let mods = self.modifiers()?;
        let ty = self.ty()?;
        let mut ty = if matches!(&ty.kind, K::Ident(n) if n == "var") { None } else { Some(ty) };
        let name = self.ident()?;
        let name_pos = self.prev_start();
        if let Some(groups) = self.dims()? {
            ty = ty.map(|t| self.add_dims(t, groups));
        }
        self.expect("=")?;
        let init = Some(Box::new(self.expr()?));
        let var = VarDecl {
            start,
            end: self.prev_end,
            mods,
            ty: ty.map(Box::new),
            name,
            init,
            is_record_component: false,
            name_pos,
        };
        Ok(self.tree(start, K::Var(Box::new(var))))
    }

    // ---- Switch ----

    fn switch(&mut self) -> P<(B, Vec<Case>)> {
        self.expect("switch")?;
        let selector = Box::new(self.parens()?);
        self.expect("{")?;
        let mut cases = Vec::new();
        let mut rules = None;
        while !self.eat("}") {
            let start = self.cur_start();
            // Like tree-sitter, each label is a case of its own (`case 1: case 2:` are two cases).
            let labels_raw = vec![self.label()?];
            let rule = self.eat("->");
            if !rule {
                self.expect(":")?;
            }
            // A switch has either rules or statement groups.
            if *rules.get_or_insert(rule) != rule {
                return self.fail();
            }
            let mut labels = Vec::new();
            let mut is_default = false;
            let mut guard = None;
            for label in labels_raw {
                if let Some(d) = label.default
                    && labels.is_empty()
                {
                    if label.default_only {
                        is_default = true;
                    } else {
                        labels.push(Tree::new(d.0, d.1, K::DefaultCaseLabel));
                    }
                }
                labels.extend(label.items);
                if let Some(g) = label.guard {
                    guard = Some(Box::new(g));
                }
                if !is_default && let Some(d) = label.default {
                    labels.retain(|l| !matches!(l.kind, K::DefaultCaseLabel));
                    labels.push(Tree::new(d.0, d.1, K::DefaultCaseLabel));
                }
            }
            let mut stmts = Vec::new();
            let mut body = None;
            if rule {
                body = Some(Box::new(match self.peek() {
                    "{" => self.block()?,
                    "throw" => self.stmt(false)?,
                    _ => {
                        let e = self.expr()?;
                        self.expect(";")?;
                        e
                    }
                }));
            } else {
                while !matches!(self.peek(), "case" | "}") && !(self.peek() == "default" && !self.at_default_modifier())
                {
                    if self.pos >= self.toks.len() {
                        return self.fail();
                    }
                    stmts.push(self.block_stmt(false)?);
                }
            }
            let kind = if rule { CaseKind::Rule } else { CaseKind::Statement };
            cases.push(Case { start, labels, is_default, guard, kind, stmts, body });
        }
        Ok((selector, cases))
    }

    /// Whether the `default` at the current token is a modifier (of an interface method in a local declaration),
    /// rather than a switch label.
    fn at_default_modifier(&self) -> bool {
        self.peek() == "default" && !matches!(self.peek_n(1), ":" | "->")
    }

    fn label(&mut self) -> P<Label> {
        let keyword_start = self.cur_start();
        if self.eat("default") {
            return Ok(Label {
                default: Some((keyword_start, self.prev_end)),
                default_only: true,
                items: vec![],
                guard: None,
            });
        }
        self.expect("case")?;
        let mut label = Label { default: None, default_only: false, items: vec![], guard: None };
        loop {
            if self.peek() == "default" {
                let s = self.cur_start();
                self.bump()?;
                label.default = Some((s, self.prev_end));
            } else if self.at_pattern() {
                label.items.push(self.pattern()?);
            } else {
                self.in_label = true;
                let item = self.ternary();
                self.in_label = false;
                label.items.push(item?);
            }
            if !self.eat(",") {
                break;
            }
        }
        if self.peek() == "when" {
            self.bump()?;
            self.in_label = true;
            let guard = self.expr();
            self.in_label = false;
            label.guard = Some(guard?);
        }
        Ok(label)
    }

    /// Whether a type or record pattern starts at the current token.
    fn at_pattern(&self) -> bool {
        let Some(i) = self.scan_modifiers(self.pos) else { return false };
        if i > self.pos {
            return true;
        }
        match self.scan_type(i) {
            // In `case C when c`, `when` starts the guard (unless it is the binding's name: `case C when ->`).
            Some(j) if self.text_at(j) == "when" => matches!(self.text_at(j + 1), "->" | ":" | "when"),
            Some(j) => is_ident(self.text_at(j)) || self.text_at(j) == "(",
            None => false,
        }
    }

    fn pattern(&mut self) -> P<Tree> {
        let start = self.cur_start();
        let mods = self.modifiers()?;
        let ty = self.ty()?;
        if self.peek() == "(" {
            self.bump()?;
            let mut nested = Vec::new();
            while !self.eat(")") {
                nested.push(self.record_pattern_component()?);
                if !self.eat(",") {
                    self.expect(")")?;
                    break;
                }
            }
            return Ok(self.tree(start, K::RecordPattern { ty: Box::new(ty), nested }));
        }
        let name = self.ident()?;
        let name_pos = self.prev_start();
        let ty = if matches!(&ty.kind, K::Ident(n) if n == "var") { None } else { Some(Box::new(ty)) };
        let var =
            VarDecl { start, end: self.prev_end, mods, ty, name, init: None, is_record_component: false, name_pos };
        Ok(self.tree(start, K::BindingPattern { var: Box::new(var) }))
    }

    fn record_pattern_component(&mut self) -> P<Tree> {
        let start = self.cur_start();
        if self.peek() == "_" && matches!(self.peek_n(1), "," | ")") {
            self.bump()?;
            return Ok(self.tree(start, K::AnyPattern));
        }
        self.pattern()
    }

    // ---- Expressions ----

    fn expr(&mut self) -> P<Tree> {
        if self.at_lambda() {
            return self.lambda();
        }
        let lhs = self.ternary()?;
        let (op, n) = self.operator();
        if ASSIGN_OPS.contains(&op) {
            self.bump_n(n)?;
            let rhs = self.expr()?;
            let start = lhs.start;
            return Ok(self.tree(start, K::Assign { op: op.into(), lhs: Box::new(lhs), rhs: Box::new(rhs) }));
        }
        Ok(lhs)
    }

    fn ternary(&mut self) -> P<Tree> {
        let cond = self.binary(4)?;
        if !self.eat("?") {
            return Ok(cond);
        }
        let then = self.expr()?;
        self.expect(":")?;
        let else_ = if self.at_lambda() { self.lambda()? } else { self.ternary()? };
        let start = cond.start;
        Ok(self.tree(start, K::Conditional { cond: Box::new(cond), then: Box::new(then), else_: Box::new(else_) }))
    }

    fn binary(&mut self, min_prec: i32) -> P<Tree> {
        let mut lhs = self.unary()?;
        loop {
            let (op, n) = self.operator();
            let Some(prec) = binary_precedence(op) else { break };
            if prec < min_prec {
                break;
            }
            self.bump_n(n)?;
            let start = lhs.start;
            if op == "instanceof" {
                let target = self.instanceof_target()?;
                lhs = self.tree(start, K::InstanceOf { expr: Box::new(lhs), target: Box::new(target) });
                continue;
            }
            let rhs = self.binary(prec + 1)?;
            lhs = self.tree(start, K::Binary { op: op.into(), lhs: Box::new(lhs), rhs: Box::new(rhs) });
        }
        Ok(lhs)
    }

    fn instanceof_target(&mut self) -> P<Tree> {
        let start = self.cur_start();
        let has_mods = self.peek() == "final";
        let mods = self.modifiers()?;
        let ty = self.ty()?;
        if self.peek() == "(" {
            // A record pattern.
            self.bump()?;
            let mut nested = Vec::new();
            while !self.eat(")") {
                nested.push(self.record_pattern_component()?);
                if !self.eat(",") {
                    self.expect(")")?;
                    break;
                }
            }
            return Ok(self.tree(start, K::RecordPattern { ty: Box::new(ty), nested }));
        }
        if is_ident(self.peek()) {
            let name = self.ident()?;
            let name_pos = self.prev_start();
            let var_start = if has_mods { mods.start } else { ty.start };
            let mods = if has_mods { mods } else { Mods { start: ty.start, annotations: vec![] } };
            let var = VarDecl {
                start: var_start,
                end: self.prev_end,
                mods,
                ty: Some(Box::new(ty)),
                name,
                init: None,
                is_record_component: false,
                name_pos,
            };
            return Ok(Tree::new(var.start, var.end, K::BindingPattern { var: Box::new(var) }));
        }
        Ok(ty)
    }

    fn unary(&mut self) -> P<Tree> {
        let start = self.cur_start();
        match self.peek() {
            op @ ("+" | "-" | "!" | "~") => {
                self.bump()?;
                if op == "-" && is_decimal_integer(self.peek()) {
                    let lit = self.bump()?;
                    let kind = if lit.ends_with(['l', 'L']) { LitKind::Long } else { LitKind::Int };
                    return Ok(self.tree(start, K::Literal(kind)));
                }
                let e = self.unary()?;
                Ok(self.tree(start, K::Unary { op: op.into(), postfix: false, expr: Box::new(e) }))
            }
            op @ ("++" | "--") => {
                self.bump()?;
                let e = self.unary()?;
                Ok(self.tree(start, K::Unary { op: op.into(), postfix: false, expr: Box::new(e) }))
            }
            "(" if !self.at_lambda() => match self.cast_end() {
                Some(usize::MAX) => self.fail(),
                Some(_) => self.cast(),
                None => self.postfix(),
            },
            _ => self.postfix(),
        }
    }

    /// If a cast starts at the current `(`, the index of its `)`.
    fn cast_end(&self) -> Option<usize> {
        let first = self.pos + 1;
        let mut i = self.scan_type(first)?;
        let mut types = 1;
        while self.text_at(i) == "&" {
            i = self.scan_type(i + 1)?;
            types += 1;
        }
        if self.text_at(i) != ")" {
            return None;
        }
        let next = self.text_at(i + 1);
        let primitive = types == 1 && i == first + 1 && is_primitive(self.text_at(first));
        let starts_operand = is_ident(next)
            || is_literal(next)
            || is_primitive(next)
            || matches!(next, "(" | "!" | "~" | "this" | "super" | "new" | "switch" | "@");
        if primitive {
            return (starts_operand || matches!(next, "+" | "-" | "++" | "--")).then_some(i);
        }
        if matches!(next, "+" | "-") {
            // `(a) + b` is an addition. tree-sitter rejects `+`/`-` after a cast to an array or parameterized type:
            // that is left to it (`usize::MAX`), so that it reports the error.
            let simple_name = (first..i).all(|k| is_ident(self.text_at(k)) || matches!(self.text_at(k), "." | "&"));
            return (!simple_name).then_some(usize::MAX);
        }
        starts_operand.then_some(i)
    }

    fn cast(&mut self) -> P<Tree> {
        let start = self.cur_start();
        self.expect("(")?;
        let mut types = vec![self.ty()?];
        while self.eat("&") {
            types.push(self.ty()?);
        }
        self.expect(")")?;
        let ty = if types.len() == 1 {
            types.pop().unwrap()
        } else {
            let (s, e) = (types[0].start, types[types.len() - 1].end);
            Tree::new(s, e, K::Intersection(types))
        };
        let expr = if self.at_lambda() { self.lambda()? } else { self.unary()? };
        Ok(self.tree(start, K::Cast { ty: Box::new(ty), expr: Box::new(expr) }))
    }

    fn postfix(&mut self) -> P<Tree> {
        let mut e = self.primary()?;
        e = self.selectors(e)?;
        while matches!(self.peek(), "++" | "--") {
            let op = self.bump()?;
            let start = e.start;
            e = self.tree(start, K::Unary { op: op.into(), postfix: true, expr: Box::new(e) });
        }
        Ok(e)
    }

    fn arguments(&mut self) -> P<Vec<Tree>> {
        // Arguments may be lambdas, also in a case label.
        let in_label = std::mem::replace(&mut self.in_label, false);
        let args = self.arguments_list();
        self.in_label = in_label;
        args
    }

    fn arguments_list(&mut self) -> P<Vec<Tree>> {
        self.expect("(")?;
        let mut args = Vec::new();
        while !self.eat(")") {
            args.push(self.expr()?);
            if !self.eat(",") {
                self.expect(")")?;
                break;
            }
        }
        Ok(args)
    }

    fn primary(&mut self) -> P<Tree> {
        let start = self.cur_start();
        let t = self.peek();
        if is_literal(t) {
            self.bump()?;
            return Ok(self.tree(start, K::Literal(literal_kind(t))));
        }
        match t {
            "(" => self.parens(),
            "this" | "super" => {
                self.bump()?;
                let ident = self.tree(start, K::Ident(t.into()));
                if self.peek() == "(" {
                    let args = self.arguments()?;
                    return Ok(
                        self.tree(start, K::MethodInvocation { select: Box::new(ident), type_args: vec![], args })
                    );
                }
                Ok(ident)
            }
            "new" => self.creator(None),
            // An annotated type before `::`: `@A T::new`.
            "@" => {
                let mut annotations = Vec::new();
                while self.at_annotation() {
                    annotations.push(self.annotation()?);
                }
                let base = self.unannotated_type()?;
                if self.peek() != "::" {
                    return self.fail();
                }
                Ok(self.annotate(start, annotations, base))
            }
            "switch" => {
                let (selector, cases) = self.switch()?;
                Ok(self.tree(start, K::SwitchExpr { selector, cases }))
            }
            "<" => {
                // `<T>this(...)`, an explicit constructor invocation with type arguments.
                let type_args = self.type_args()?;
                let name_start = self.cur_start();
                let name = self.bump()?;
                if !matches!(name, "this" | "super") {
                    return self.fail();
                }
                let select = self.tree(name_start, K::Ident(name.into()));
                let args = self.arguments()?;
                Ok(self.tree(start, K::MethodInvocation { select: Box::new(select), type_args, args }))
            }
            _ if is_primitive(t) || self.at_type_primary() => self.unannotated_type(),
            _ if is_ident(t) => {
                self.bump()?;
                let ident = self.tree(start, K::Ident(t.into()));
                if self.peek() == "(" {
                    let args = self.arguments()?;
                    return Ok(
                        self.tree(start, K::MethodInvocation { select: Box::new(ident), type_args: vec![], args })
                    );
                }
                Ok(ident)
            }
            _ => self.fail(),
        }
    }

    /// Whether a type with type arguments or dimensions starts here, followed by `::` or `.class`.
    fn at_type_primary(&self) -> bool {
        if !is_ident(self.peek()) {
            return false;
        }
        let mut i = self.pos + 1;
        while self.text_at(i) == "." && is_ident(self.text_at(i + 1)) {
            i += 2;
        }
        if !matches!(self.text_at(i), "<" | "[") {
            return false;
        }
        match self.scan_type(self.pos) {
            Some(j) if j > i => self.text_at(j) == "::" || (self.text_at(j) == "." && self.text_at(j + 1) == "class"),
            _ => false,
        }
    }

    fn selectors(&mut self, mut e: Tree) -> P<Tree> {
        loop {
            let start = e.start;
            match self.peek() {
                "." => {
                    let next = self.peek_n(1);
                    match next {
                        "new" => {
                            self.bump()?;
                            e = self.creator(Some(e))?;
                        }
                        "class" | "this" => {
                            self.bump_n(2)?;
                            e = self.tree(start, K::MemberSelect { expr: Box::new(e), name: next.into() });
                        }
                        "super" => {
                            self.bump_n(2)?;
                            e = self.tree(start, K::MemberSelect { expr: Box::new(e), name: "super".into() });
                            if self.peek() == "(" {
                                let args = self.arguments()?;
                                e = self
                                    .tree(start, K::MethodInvocation { select: Box::new(e), type_args: vec![], args });
                            }
                        }
                        "<" => {
                            self.bump()?;
                            let type_args = self.type_args()?;
                            let name = self.bump()?;
                            if !(is_ident(name) || matches!(name, "super" | "this")) {
                                return self.fail();
                            }
                            let select = self.tree(start, K::MemberSelect { expr: Box::new(e), name: name.into() });
                            let args = self.arguments()?;
                            e = self.tree(start, K::MethodInvocation { select: Box::new(select), type_args, args });
                        }
                        _ if is_ident(next) => {
                            self.bump()?;
                            let name = self.ident()?;
                            let select = self.tree(start, K::MemberSelect { expr: Box::new(e), name });
                            e = if self.peek() == "(" {
                                let args = self.arguments()?;
                                self.tree(
                                    start,
                                    K::MethodInvocation { select: Box::new(select), type_args: vec![], args },
                                )
                            } else {
                                select
                            };
                        }
                        _ => return self.fail(),
                    }
                }
                "[" => {
                    self.bump()?;
                    let index = self.expr()?;
                    self.expect("]")?;
                    e = self.tree(start, K::ArrayAccess { expr: Box::new(e), index: Box::new(index) });
                }
                "::" => {
                    self.bump()?;
                    let type_args = if self.peek() == "<" { self.type_args()? } else { vec![] };
                    let name = if self.eat("new") { None } else { Some(self.ident()?) };
                    e = self.tree(start, K::MemberRef { qual: Box::new(e), type_args, name });
                }
                _ => return Ok(e),
            }
        }
    }

    fn creator(&mut self, outer: Option<Tree>) -> P<Tree> {
        let start = outer.as_ref().map_or(self.cur_start(), |o| o.start);
        self.expect("new")?;
        let type_args = if self.peek() == "<" { self.type_args()? } else { vec![] };
        let annotations_start = self.cur_start();
        let mut annotations = Vec::new();
        while self.at_annotation() {
            annotations.push(self.annotation()?);
        }
        let ty = self.element_type()?;
        let ty = if annotations.is_empty() {
            ty
        } else {
            Tree::new(annotations_start, ty.end, K::Annotated { annotations, base: Box::new(ty) })
        };
        let at_dims = self.scan_annotations(self.pos).is_some_and(|j| self.text_at(j) == "[");
        if at_dims {
            let mut dims = Vec::new();
            let mut dim_annotations = Vec::new();
            while let Some(j) = self.scan_annotations(self.pos) {
                if self.text_at(j) != "[" {
                    break;
                }
                let mut annos = Vec::new();
                while self.at_annotation() {
                    annos.push(self.annotation()?);
                }
                self.expect("[")?;
                if !self.eat("]") {
                    dims.push(self.expr()?);
                    self.expect("]")?;
                }
                dim_annotations.push(annos);
            }
            let init = if self.peek() == "{" {
                match self.array_initializer()?.take_kind() {
                    K::NewArray { init, .. } => init,
                    _ => None,
                }
            } else {
                None
            };
            return Ok(self.tree(start, K::NewArray { elem: Some(Box::new(ty)), dims, dim_annotations, init }));
        }
        let args = self.arguments()?;
        let body = if self.peek() == "{" { Some(self.class_body(ClassKind::Class)?) } else { None };
        Ok(self.tree(start, K::NewClass { outer: outer.map(Box::new), type_args, ident: Box::new(ty), args, body }))
    }

    fn lambda(&mut self) -> P<Tree> {
        let start = self.cur_start();
        let params = if is_ident(self.peek()) {
            let s = self.cur_start();
            let name = self.ident()?;
            vec![inferred_param(s, self.prev_end, name)]
        } else {
            self.expect("(")?;
            let mut params = Vec::new();
            let inferred = is_ident(self.peek()) && matches!(self.peek_n(1), "," | ")");
            while !self.eat(")") {
                if inferred {
                    let s = self.cur_start();
                    let name = self.ident()?;
                    params.push(inferred_param(s, self.prev_end, name));
                } else {
                    params.push(self.formal_parameter()?);
                }
                if !self.eat(",") {
                    self.expect(")")?;
                    break;
                }
            }
            params
        };
        self.expect("->")?;
        let body = if self.peek() == "{" { self.block()? } else { self.expr()? };
        Ok(self.tree(start, K::Lambda { params, body: Box::new(body) }))
    }
}

fn inferred_param(start: usize, end: usize, name: String) -> VarDecl {
    let name_pos = start;
    VarDecl {
        start,
        end,
        mods: Mods { start, annotations: vec![] },
        ty: None,
        name,
        init: None,
        is_record_component: false,
        name_pos,
    }
}

/// `this(...)`, `super(...)` or `outer.super(...)`.
fn is_constructor_call(e: &Tree) -> bool {
    let K::MethodInvocation { select, .. } = &e.kind else { return false };
    match &select.kind {
        K::Ident(name) => name == "this" || name == "super",
        K::MemberSelect { name, .. } => name == "super",
        _ => false,
    }
}
