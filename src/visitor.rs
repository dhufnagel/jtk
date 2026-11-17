// Derived from palantir-java-format's JavaInputAstVisitor.java, Java14InputAstVisitor.java and Java21InputAstVisitor.java
// Copyright 2015-2019 Google Inc.; Copyright 2019-2025 Palantir Technologies Inc.
// Modified by the jtk authors: ported to Rust. Licensed under the Apache License, Version 2.0.

//! Port of `JavaInputAstVisitor` (including the Java 14 and Java 21 extensions): walks the AST and emits ops.

use std::collections::VecDeque;

use crate::ast::{
    Case, CaseKind, Catch, ClassDecl, ClassKind, EnumConstant, K, LitKind, MethodDecl, Mods, Tree, VarDecl,
};
use crate::doc::{
    BreakBehaviour, BreakDoc, BreakTag, FillMode, Indent, LastLevelBreakability, Op, OpenOp, PartialInlineability,
    RealOrImaginary,
};
use crate::ops::{BlankLineWanted, OpsBuilder};

const METHOD_CHAIN_COLUMN_LIMIT: i32 = 80;
const ANNOTATION_RECORD_PARAMETER_COLUMN_LIMIT: i32 = 60;
const MAX_ITEM_LENGTH_FOR_FILLING: usize = 10;

const ZERO: Indent = Indent::Const(0);

const LOG_METHODS: &[&str] = &[
    "at",
    "atConfig",
    "atDebug",
    "atFine",
    "atFiner",
    "atFinest",
    "atInfo",
    "atMostEvery",
    "atSevere",
    "atWarning",
    "every",
    "log",
    "logVarargs",
    "perUnique",
    "withCause",
    "withStackTrace",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeclKind {
    None,
    Field,
    Parameter,
}

fn prefer_breaking_last(keep: bool) -> BreakBehaviour {
    BreakBehaviour::PreferBreakingLastInnerLevel { keep_indent_when_inlined: keep }
}

fn break_only_if_inner(keep: bool) -> BreakBehaviour {
    BreakBehaviour::BreakOnlyIfInnerLevelsThenFitOnOneLine { keep_indent_when_inlined: keep }
}

fn break_list(tag: Option<BreakTag>) -> Vec<Op> {
    vec![Op::Break(BreakDoc::new(FillMode::Unified, " ", ZERO, tag))]
}

fn break_fill_list(tag: Option<BreakTag>) -> Vec<Op> {
    vec![Op::Open(OpenOp::new(ZERO)), Op::Break(BreakDoc::new(FillMode::Independent, " ", ZERO, tag)), Op::Close]
}

fn force_break_list(tag: Option<BreakTag>) -> Vec<Op> {
    vec![Op::Break(BreakDoc::new(FillMode::Forced, "", ZERO, tag))]
}

/// javac's `TreeInfo.opPrec`.
fn precedence(op: &str) -> i32 {
    match op {
        "||" => 4,
        "&&" => 5,
        "|" => 6,
        "^" => 7,
        "&" => 8,
        "==" | "!=" => 9,
        "<" | ">" | "<=" | ">=" => 10,
        "<<" | ">>" | ">>>" => 11,
        "+" | "-" => 12,
        _ => 13,
    }
}

fn array_base(mut node: &Tree) -> &Tree {
    while let K::ArrayAccess { expr, .. } = &node.kind {
        node = expr;
    }
    node
}

fn array_indices(mut node: &Tree) -> Vec<&Tree> {
    let mut indices = Vec::new();
    while let K::ArrayAccess { expr, index } = &node.kind {
        indices.push(&**index);
        node = expr;
    }
    indices
}

fn method_name(select: &Tree) -> &str {
    match &select.kind {
        K::MemberSelect { name, .. } => name,
        K::Ident(name) => name,
        _ => "",
    }
}

fn method_receiver(select: &Tree) -> Option<&Tree> {
    match &select.kind {
        K::MemberSelect { expr, .. } => Some(expr),
        _ => None,
    }
}

/// Returns `(base type, dims)` for a possibly-array type.
fn extract_dims(t: &Tree) -> (&Tree, Vec<&Vec<Tree>>) {
    match &t.kind {
        K::ArrayType { elem, dims } => (elem, dims.iter().collect()),
        _ => (t, Vec::new()),
    }
}

fn is_block(t: &Tree) -> bool {
    matches!(t.kind, K::Block { .. })
}

fn inline_annotation_direction(mods: &Mods) -> Direction {
    for a in &mods.annotations {
        if let K::Annotation { args, .. } = &a.kind
            && !args.is_empty()
        {
            return Direction::Vertical;
        }
    }
    Direction::Horizontal
}

fn can_local_have_horizontal_annotations(mods: &Mods) -> Direction {
    let markers =
        mods.annotations.iter().filter(|a| matches!(&a.kind, K::Annotation { args, .. } if args.is_empty())).count();
    if markers <= 1 && markers == mods.annotations.len() { Direction::Horizontal } else { Direction::Vertical }
}

/// Classifies qualified names as types (port of `TypeNameClassifier`).
mod type_names {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Case {
        Uppercase,
        Lowercase,
        UpperCamel,
        LowerCamel,
    }

    fn case_of(name: &str) -> Case {
        let mut first_upper = false;
        let mut has_upper = false;
        let mut has_lower = false;
        let mut first = true;
        for c in name.chars() {
            if !c.is_alphabetic() {
                continue;
            }
            if first {
                first_upper = c.is_uppercase();
                first = false;
            }
            has_upper |= c.is_uppercase();
            has_lower |= c.is_lowercase();
        }
        if first_upper {
            if has_lower { Case::UpperCamel } else { Case::Uppercase }
        } else if has_upper {
            Case::LowerCamel
        } else {
            Case::Lowercase
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum St {
        Start,
        Type,
        FirstStaticMember,
        Reject,
        Ambiguous,
    }

    pub fn type_prefix_length(parts: &[String]) -> Option<usize> {
        let mut state = St::Start;
        let mut result = None;
        for (i, p) in parts.iter().enumerate() {
            let c = case_of(p);
            state = match state {
                St::Start => match c {
                    Case::Uppercase => St::Ambiguous,
                    Case::LowerCamel => St::Reject,
                    Case::Lowercase => St::Start,
                    Case::UpperCamel => St::Type,
                },
                St::Type => match c {
                    Case::UpperCamel => St::Type,
                    _ => St::FirstStaticMember,
                },
                St::FirstStaticMember | St::Reject => St::Reject,
                St::Ambiguous => match c {
                    Case::Uppercase => St::Ambiguous,
                    Case::LowerCamel | Case::Lowercase => St::Reject,
                    Case::UpperCamel => St::Type,
                },
            };
            if state == St::Reject {
                break;
            }
            if matches!(state, St::Type | St::FirstStaticMember) {
                result = Some(i);
            }
        }
        result
    }
}

pub struct Visitor<'a> {
    pub b: OpsBuilder<'a>,
    src: &'a str,
    /// For each scanned tree on the current path: whether it is an annotation.
    path: Vec<bool>,
    /// For each scanned tree on the current path: whether we are inside an expression.
    in_expression: Vec<bool>,
    indent_multiplier: i32,
    plus_two: Indent,
    plus_four: Indent,
    minus_four: Indent,
}

impl<'a> Visitor<'a> {
    /// `indent_multiplier` scales all indents: 2 for the Palantir style (4-space blocks), 1 for the Google style.
    pub fn new(b: OpsBuilder<'a>, src: &'a str, indent_multiplier: i32) -> Visitor<'a> {
        Visitor {
            b,
            src,
            path: Vec::new(),
            in_expression: vec![false],
            indent_multiplier,
            plus_two: Indent::Const(2 * indent_multiplier),
            plus_four: Indent::Const(4 * indent_multiplier),
            minus_four: Indent::Const(-4 * indent_multiplier),
        }
    }

    fn token(&mut self, t: &str) {
        self.b.token(t, RealOrImaginary::Real, ZERO, None);
    }

    fn token_indent(&mut self, t: &str, plus_indent_comments_before: Indent) {
        self.b.token(t, RealOrImaginary::Real, plus_indent_comments_before, None);
    }

    fn token_break_trailing_comment(&mut self, t: &str, indent: Indent) {
        self.b.token(t, RealOrImaginary::Real, ZERO, Some(indent));
    }

    fn sync(&mut self, pos: usize) {
        self.b.sync(pos);
    }

    fn source(&self, t: &Tree) -> &'a str {
        &self.src[t.start..t.end]
    }

    fn drop_empty_declarations(&mut self) {
        while self.b.peek_is(";") {
            self.b.forced_break();
            self.mark_for_partial_format();
            self.token(";");
        }
    }

    fn mark_for_partial_format(&mut self) {
        if !self.in_expression.last().copied().unwrap_or(false) {
            self.b.mark_for_partial_format();
        }
    }

    pub fn scan(&mut self, t: &Tree) {
        let in_expression = t.is_expression() || self.in_expression.last().copied().unwrap_or(false);
        self.in_expression.push(in_expression);
        self.path.push(matches!(t.kind, K::Annotation { .. }));
        self.visit(t);
        self.path.pop();
        self.in_expression.pop();
    }

    fn visit(&mut self, t: &Tree) {
        match &t.kind {
            K::CompilationUnit { package, imports, types, module } => {
                self.visit_compilation_unit(package, imports, types, module.as_deref())
            }
            K::Import { is_static, name } => {
                self.sync(t.start);
                self.token("import");
                self.b.space();
                if *is_static {
                    self.token("static");
                    self.b.space();
                }
                self.visit_name(name);
                self.token(";");
                self.drop_empty_declarations();
            }
            K::Class(c) => {
                self.sync(t.start);
                match c.kind {
                    ClassKind::Annotation => self.visit_annotation_type(c),
                    ClassKind::Class | ClassKind::Interface => self.visit_class_declaration(c),
                    ClassKind::Enum => self.visit_enum_declaration(c),
                    ClassKind::Record => self.visit_record_declaration(c),
                }
            }
            K::Method(m) => {
                self.sync(t.start);
                self.visit_method(m);
            }
            K::Vars(vars) => {
                self.sync(t.start);
                let refs: Vec<&VarDecl> = vars.iter().collect();
                self.visit_variables(&refs, DeclKind::None, inline_annotation_direction(&vars[0].mods));
            }
            K::Var(v) => {
                self.sync(t.start);
                self.visit_variables(&[v], DeclKind::None, inline_annotation_direction(&v.mods));
            }
            K::Block { .. } => self.visit_block(t, false, false, false),
            K::Empty => {
                self.sync(t.start);
                self.drop_empty_declarations();
            }
            K::ExprStmt(e) => {
                self.sync(t.start);
                self.scan(e);
                self.token(";");
            }
            K::If { .. } => self.visit_if(t),
            K::While { cond, body } => {
                self.sync(t.start);
                self.token("while");
                self.b.space();
                self.token("(");
                self.scan(skip_paren(cond));
                self.token(")");
                self.visit_statement(body, true, true, false);
            }
            K::DoWhile { body, cond } => {
                self.sync(t.start);
                self.token("do");
                self.visit_statement(body, true, true, true);
                if is_block(body) {
                    self.b.space();
                } else {
                    self.b.break_flat(" ");
                }
                self.token("while");
                self.b.space();
                self.token("(");
                self.scan(skip_paren(cond));
                self.token(")");
                self.token(";");
            }
            K::For { init, cond, update, body } => self.visit_for(t, init, cond.as_deref(), update, body),
            K::ForEach { var, expr, body } => {
                self.sync(t.start);
                self.b.open(ZERO);
                self.token("for");
                self.b.space();
                self.token("(");
                self.b.open(ZERO);
                self.visit_to_declare(DeclKind::None, Direction::Horizontal, var, Some(expr), ":", None, None);
                self.b.close();
                self.token(")");
                self.b.close();
                self.visit_statement(body, true, true, false);
            }
            K::Labeled { label, body } => {
                self.sync(t.start);
                self.b.open(ZERO);
                self.token(label);
                self.token(":");
                self.b.forced_break();
                self.b.close();
                self.scan(body);
            }
            K::Switch { selector, cases } | K::SwitchExpr { selector, cases } => {
                self.sync(t.start);
                self.visit_switch(selector, cases);
            }
            K::Try { resources, block, catches, finally } => {
                self.visit_try(t, resources, block, catches, finally.as_deref())
            }
            K::Throw(e) => {
                self.sync(t.start);
                self.token("throw");
                self.b.space();
                self.scan(e);
                self.token(";");
            }
            K::Return(e) => {
                self.sync(t.start);
                self.token("return");
                if let Some(e) = e {
                    self.b.space();
                    self.scan(e);
                }
                self.token(";");
            }
            K::Break(label) => {
                self.sync(t.start);
                self.b.open(self.plus_four.clone());
                self.token("break");
                if let Some(l) = label {
                    self.b.break_flat(" ");
                    self.token(l);
                }
                self.b.close();
                self.token(";");
            }
            K::Continue(label) => {
                self.sync(t.start);
                self.b.open(self.plus_four.clone());
                self.token("continue");
                if let Some(l) = label {
                    self.b.break_flat(" ");
                    self.token(l);
                }
                self.token(";");
                self.b.close();
            }
            K::Synchronized { lock, block } => {
                self.sync(t.start);
                self.token("synchronized");
                self.b.space();
                self.token("(");
                self.b.open(self.plus_four.clone());
                self.b.break_op();
                self.scan(skip_paren(lock));
                self.b.close();
                self.token(")");
                self.b.space();
                self.scan(block);
            }
            K::Assert { cond, detail } => {
                self.sync(t.start);
                self.b.open(ZERO);
                self.token("assert");
                self.b.space();
                self.b.open(if detail.is_none() { ZERO } else { self.plus_four.clone() });
                self.scan(cond);
                if let Some(d) = detail {
                    self.b.break_flat(" ");
                    self.token(":");
                    self.b.space();
                    self.scan(d);
                }
                self.b.close();
                self.b.close();
                self.token(";");
            }
            K::Yield(e) => {
                self.sync(t.start);
                self.token("yield");
                self.b.space();
                self.scan(e);
                self.token(";");
            }
            K::Ident(name) => {
                self.sync(t.start);
                self.token(name);
            }
            K::MemberSelect { .. } | K::ArrayAccess { .. } => {
                self.sync(t.start);
                self.visit_dot(t);
            }
            K::MethodInvocation { .. } => {
                self.sync(t.start);
                if !self.handle_log_statement(t) {
                    self.visit_dot(t);
                }
            }
            K::Literal(_) => {
                self.sync(t.start);
                let mut s = self.source(t);
                // A negative decimal literal is a single literal in javac, but two tokens.
                if let Some(rest) = s.strip_prefix('-') {
                    self.token("-");
                    s = rest.trim();
                }
                self.token(s);
            }
            K::Binary { .. } => self.visit_binary(t),
            K::Unary { op, postfix, expr } => {
                self.sync(t.start);
                if *postfix {
                    self.scan(expr);
                    self.split_token(op);
                } else {
                    self.split_token(op);
                    if ambiguous_unary_operator(op, expr) {
                        self.b.space();
                    }
                    self.scan(expr);
                }
            }
            K::Assign { op, lhs, rhs } => {
                self.sync(t.start);
                self.b.open_op(OpenOp::new(self.plus_four.clone()).behaviour(break_only_if_inner(false)));
                self.scan(lhs);
                self.b.space();
                self.split_token(op);
                self.b.break_flat(" ");
                self.scan(rhs);
                self.b.close();
            }
            K::Conditional { cond, then, else_ } => {
                self.sync(t.start);
                self.b.open(self.plus_four.clone());
                self.scan(cond);
                self.b.break_flat(" ");
                self.token("?");
                self.b.space();
                self.scan(then);
                self.b.break_flat(" ");
                self.token(":");
                self.b.space();
                self.scan(else_);
                self.b.close();
            }
            K::InstanceOf { expr, target } => {
                self.sync(t.start);
                self.b.open(self.plus_four.clone());
                self.scan(expr);
                self.b.break_flat(" ");
                self.b.open(ZERO);
                self.token("instanceof");
                self.b.break_flat(" ");
                self.scan(target);
                self.b.close();
                self.b.close();
            }
            K::Cast { ty, expr } => {
                self.sync(t.start);
                self.b.open_op(
                    OpenOp::new(self.plus_four.clone())
                        .behaviour(prefer_breaking_last(true))
                        .breakability(LastLevelBreakability::AcceptInlineChain),
                );
                self.token("(");
                self.scan(ty);
                self.token(")");
                self.b.break_flat(" ");
                self.scan(expr);
                self.b.close();
            }
            K::Parens(e) => {
                self.token("(");
                self.scan(e);
                self.token(")");
            }
            K::NewClass { outer, type_args, ident, args, body } => {
                self.visit_new_class(t, outer.as_deref(), type_args, ident, args, body.as_deref())
            }
            K::NewArray { elem, dims, dim_annotations, init } => {
                self.visit_new_array(elem.as_deref(), dims, dim_annotations, init.as_deref())
            }
            K::Lambda { params, body } => {
                self.sync(t.start);
                self.visit_lambda(params, body);
            }
            K::MemberRef { qual, type_args, name } => {
                self.sync(t.start);
                self.b.open_op(
                    OpenOp::new(self.plus_four.clone())
                        .name("methodReference")
                        .breakability(LastLevelBreakability::CheckInner)
                        .behaviour(BreakBehaviour::InlineSuffix),
                );
                self.scan(qual);
                self.b.open(ZERO);
                self.b.break_op();
                self.b.op("::");
                self.add_type_arguments(type_args, self.plus_four.clone());
                match name {
                    Some(n) => self.token(n),
                    None => self.token("new"),
                }
                self.b.close();
                self.b.close();
            }
            K::Primitive(name) => {
                self.sync(t.start);
                self.token(name);
            }
            K::ArrayType { .. } => {
                self.sync(t.start);
                self.visit_annotated_array_type(t);
            }
            K::Parameterized { base, args } => {
                self.sync(t.start);
                if args.is_empty() {
                    self.scan(base);
                    self.token("<");
                    self.token(">");
                } else {
                    self.b.open(self.plus_four.clone());
                    self.scan(base);
                    self.token("<");
                    self.b.break_op();
                    self.b.open(ZERO);
                    let mut first = true;
                    for a in args {
                        if !first {
                            self.token(",");
                            self.b.break_flat(" ");
                        }
                        self.scan(a);
                        first = false;
                    }
                    self.b.close();
                    self.b.close();
                    self.token(">");
                }
            }
            K::Wildcard { bound } => {
                self.sync(t.start);
                self.b.open(ZERO);
                self.token("?");
                if let Some((extends, bound)) = bound {
                    self.b.open(self.plus_four.clone());
                    self.b.space();
                    self.token(if *extends { "extends" } else { "super" });
                    self.b.break_flat(" ");
                    self.scan(bound);
                    self.b.close();
                }
                self.b.close();
            }
            K::Annotated { annotations, base } => {
                self.sync(t.start);
                if let K::MemberSelect { expr, name } = &base.kind {
                    self.scan(expr);
                    self.token(".");
                    self.visit_annotations(annotations, false, false);
                    self.b.break_to_fill(" ");
                    self.token(name);
                } else {
                    self.visit_annotations(annotations, false, false);
                    self.b.break_to_fill(" ");
                    self.scan(base);
                }
            }
            K::Union(_) => {
                self.b.error = Some(crate::formatter::FormatError::new("expected manual descent into union types"))
            }
            K::Intersection(bounds) => {
                self.sync(t.start);
                self.b.open(self.plus_four.clone());
                let mut first = true;
                for ty in bounds {
                    if !first {
                        self.b.break_to_fill(" ");
                        self.token("&");
                        self.b.space();
                    }
                    self.scan(ty);
                    first = false;
                }
                self.b.close();
            }
            K::Annotation { .. } => self.visit_annotation(t),
            K::TypeParameter { annotations, name, bounds } => {
                self.sync(t.start);
                self.b.open(ZERO);
                self.visit_annotations(annotations, false, true);
                self.token(name);
                if !bounds.is_empty() {
                    self.b.space();
                    self.token("extends");
                    self.b.open(self.plus_four.clone());
                    self.b.break_flat(" ");
                    self.b.open(self.plus_four.clone());
                    let mut first = true;
                    for bound in bounds {
                        if !first {
                            self.b.break_to_fill(" ");
                            self.token("&");
                            self.b.space();
                        }
                        self.scan(bound);
                        first = false;
                    }
                    self.b.close();
                    self.b.close();
                }
                self.b.close();
            }
            K::BindingPattern { var } => {
                self.sync(t.start);
                self.b.open(self.plus_four.clone());
                let ops = self.visit_modifiers(&var.mods.annotations, Direction::Horizontal, None, false);
                self.b.add_all(ops);
                match &var.ty {
                    None => self.token("var"),
                    Some(ty) => self.scan(ty),
                }
                self.b.break_flat(" ");
                self.token(&var.name);
                self.b.close();
            }
            K::AnyPattern => {
                self.sync(t.start);
                self.token("_");
            }
            K::RecordPattern { ty, nested } => {
                self.scan(ty);
                self.b.open(self.plus_four.clone());
                self.token("(");
                self.b.break_op();
                let mut first = true;
                for p in nested {
                    if !first {
                        self.token(",");
                        self.b.break_flat(" ");
                    }
                    first = false;
                    self.scan(p);
                }
                self.b.close();
                self.token(")");
            }
            K::DefaultCaseLabel => self.token("default"),
            K::Module { annotations, open, name, directives } => {
                self.visit_module(annotations, *open, name, directives)
            }
            K::Directive { keyword, name, separator, items } => {
                self.token(keyword);
                self.b.space();
                self.scan(name);
                if let Some(items) = items {
                    self.b.open(self.plus_four.clone());
                    self.b.space();
                    self.token(separator);
                    self.b.forced_break();
                    let mut first = true;
                    for item in items {
                        if !first {
                            self.token(",");
                            self.b.forced_break();
                        }
                        self.scan(item);
                        first = false;
                    }
                    self.token(";");
                    self.b.close();
                } else {
                    self.token(";");
                }
            }
            K::Requires { name } => {
                self.token("requires");
                self.b.space();
                loop {
                    if self.b.peek_is("static") {
                        self.token("static");
                        self.b.space();
                    } else if self.b.peek_is("transitive") {
                        self.token("transitive");
                        self.b.space();
                    } else {
                        break;
                    }
                }
                self.scan(name);
                self.token(";");
            }
            K::Uses { name } => {
                self.token("uses");
                self.b.space();
                self.scan(name);
                self.token(";");
            }
        }
    }

    fn split_token(&mut self, op: &str) {
        for i in 0..op.len() {
            self.token(&op[i..i + 1]);
        }
    }

    // ------------------------------------------------------------------------------------------------------------
    // Compilation units, packages, imports, modules
    // ------------------------------------------------------------------------------------------------------------

    fn visit_compilation_unit(
        &mut self,
        package: &Option<(Vec<Tree>, Box<Tree>)>,
        imports: &[Tree],
        types: &[Tree],
        module: Option<&Tree>,
    ) {
        let mut first = true;
        if let Some((annotations, name)) = package {
            self.mark_for_partial_format();
            if !annotations.is_empty() {
                for a in annotations {
                    self.b.forced_break();
                    self.scan(a);
                }
                self.b.forced_break();
            }
            self.b.open(self.plus_four.clone());
            self.token("package");
            self.b.space();
            self.visit_name(name);
            self.b.close();
            self.token(";");
            self.b.forced_break();
            first = false;
        }
        self.drop_empty_declarations();
        if !imports.is_empty() {
            if !first {
                self.b.blank_line_wanted(BlankLineWanted::Yes);
            }
            for imp in imports {
                self.mark_for_partial_format();
                self.b.blank_line_wanted(BlankLineWanted::Preserve);
                self.scan(imp);
                self.b.forced_break();
            }
            first = false;
        }
        self.drop_empty_declarations();
        for ty in types {
            if !first {
                self.b.blank_line_wanted(BlankLineWanted::Yes);
            }
            self.mark_for_partial_format();
            self.scan(ty);
            self.b.forced_break();
            first = false;
            self.drop_empty_declarations();
        }
        if let Some(m) = module {
            if !first {
                self.b.blank_line_wanted(BlankLineWanted::Yes);
            }
            self.mark_for_partial_format();
            self.scan(m);
            self.b.forced_break();
        }
        // A partial format marker at EOF makes sure the entire file can be formatted.
        self.mark_for_partial_format();
        self.b.forced_break();
    }

    fn visit_name(&mut self, node: &Tree) {
        let mut parts = Vec::new();
        let mut n = node;
        while let K::MemberSelect { expr, name } = &n.kind {
            parts.push(name.as_str());
            n = expr;
        }
        if let K::Ident(name) = &n.kind {
            parts.push(name);
        }
        parts.reverse();
        let mut first = true;
        for p in parts {
            if !first {
                self.token(".");
            }
            self.token(p);
            first = false;
        }
    }

    fn visit_module(&mut self, annotations: &[Tree], open: bool, name: &Tree, directives: &[Tree]) {
        for a in annotations {
            self.scan(a);
            self.b.forced_break();
        }
        if open {
            self.token("open");
            self.b.space();
        }
        self.token("module");
        self.b.space();
        self.scan(name);
        self.b.space();
        if directives.is_empty() {
            self.token_break_trailing_comment("{", self.plus_two.clone());
            self.b.blank_line_wanted(BlankLineWanted::No);
            self.token_indent("}", self.plus_two.clone());
        } else {
            self.b.open(self.plus_two.clone());
            self.token("{");
            self.b.forced_break();
            let mut previous: Option<String> = None;
            for d in directives {
                self.mark_for_partial_format();
                let kind = d.javac_kind() + directive_keyword(d);
                let wanted = previous.as_ref().is_some_and(|p| *p != kind);
                self.b.blank_line_wanted(if wanted { BlankLineWanted::Yes } else { BlankLineWanted::No });
                self.b.forced_break();
                self.scan(d);
                previous = Some(kind);
            }
            self.b.close();
            self.b.forced_break();
            self.token("}");
        }
    }

    // ------------------------------------------------------------------------------------------------------------
    // Type declarations
    // ------------------------------------------------------------------------------------------------------------

    fn visit_annotation_type(&mut self, c: &ClassDecl) {
        self.b.open(ZERO);
        self.visit_and_break_modifiers(&c.mods, false, Direction::Vertical, None);
        self.b.open(ZERO);
        self.token("@");
        self.token("interface");
        self.b.break_flat(" ");
        self.token(&c.name);
        self.b.close();
        self.b.close();
        self.add_body_declarations(&c.members, true, true);
        self.b.guess_token(";");
    }

    fn visit_class_declaration(&mut self, c: &ClassDecl) {
        let breaks = self.visit_modifiers(&c.mods.annotations, Direction::Vertical, None, false);
        let has_superclass = c.extends.is_some();
        let has_super_interfaces = !c.implements.is_empty();
        let has_permits = !c.permits.is_empty();
        self.b.add_all(breaks);
        self.token(if c.kind == ClassKind::Interface { "interface" } else { "class" });
        self.b.space();
        self.token(&c.name);
        if !c.type_params.is_empty() {
            self.token("<");
        }
        self.b.open(self.plus_four.clone());
        if !c.type_params.is_empty() {
            let indent =
                if has_superclass || has_super_interfaces || has_permits { self.plus_four.clone() } else { ZERO };
            self.type_parameters_rest(&c.type_params, indent);
        }
        if let Some(ext) = &c.extends {
            self.b.break_to_fill(" ");
            self.token("extends");
            self.b.space();
            self.scan(ext);
        }
        self.class_declaration_type_list(
            if c.kind == ClassKind::Interface { "extends" } else { "implements" },
            &c.implements,
        );
        self.class_declaration_type_list("permits", &c.permits);
        self.b.close();
        self.add_body_declarations(&c.members, true, true);
        self.drop_empty_declarations();
    }

    fn visit_record_declaration(&mut self, c: &ClassDecl) {
        let breaks = self.visit_modifiers(&c.mods.annotations, Direction::Vertical, None, false);
        let has_super_interfaces = !c.implements.is_empty();
        self.b.add_all(breaks);
        self.token("record");
        self.b.space();
        self.token(&c.name);
        if !c.type_params.is_empty() {
            self.token("<");
        }
        self.b.open(self.plus_four.clone());
        if !c.type_params.is_empty() {
            self.type_parameters_rest(&c.type_params, if has_super_interfaces { self.plus_four.clone() } else { ZERO });
        }
        self.token("(");
        if !c.record_components.is_empty() {
            self.b.break_to_fill("");
        }
        let params: Vec<&VarDecl> = c.record_components.iter().collect();
        self.visit_formals(None, &params);
        self.token(")");
        if has_super_interfaces {
            self.b.break_to_fill(" ");
            self.b.open(if c.implements.len() > 1 { self.plus_four.clone() } else { ZERO });
            self.token("implements");
            self.b.space();
            let mut first = true;
            for t in &c.implements {
                if !first {
                    self.token(",");
                    self.b.break_flat(" ");
                }
                self.scan(t);
                first = false;
            }
            self.b.close();
        }
        self.b.close();
        self.add_body_declarations(&c.members, true, true);
        self.drop_empty_declarations();
    }

    fn visit_enum_declaration(&mut self, c: &ClassDecl) {
        self.b.open(ZERO);
        self.visit_and_break_modifiers(&c.mods, false, Direction::Vertical, None);
        self.b.open(self.plus_four.clone());
        self.token("enum");
        self.b.break_flat(" ");
        self.token(&c.name);
        self.b.close();
        self.b.close();
        if !c.implements.is_empty() {
            self.b.open(self.plus_four.clone());
            self.b.break_flat(" ");
            self.b.open(self.plus_four.clone());
            self.token("implements");
            self.b.break_flat(" ");
            self.b.open(ZERO);
            let mut first = true;
            for t in &c.implements {
                if !first {
                    self.token(",");
                    self.b.break_to_fill(" ");
                }
                self.scan(t);
                first = false;
            }
            self.b.close();
            self.b.close();
            self.b.close();
        }
        self.b.space();
        self.token_break_trailing_comment("{", self.plus_two.clone());
        if c.enum_constants.is_empty() && c.members.is_empty() {
            if self.b.peek_is(";") {
                self.b.open(self.plus_two.clone());
                self.b.forced_break();
                self.token(";");
                self.b.forced_break();
                self.drop_empty_declarations();
                self.b.close();
                self.b.open(ZERO);
                self.b.forced_break();
                self.b.blank_line_wanted(BlankLineWanted::No);
                self.token_indent("}", self.plus_two.clone());
                self.b.close();
            } else {
                self.b.open(ZERO);
                self.b.blank_line_wanted(BlankLineWanted::No);
                self.token("}");
                self.b.close();
            }
        } else {
            self.b.open(self.plus_two.clone());
            self.b.blank_line_wanted(BlankLineWanted::No);
            self.b.forced_break();
            self.b.open(ZERO);
            let mut first = true;
            for ec in &c.enum_constants {
                if !first {
                    self.token(",");
                    self.b.forced_break();
                    self.b.blank_line_wanted(BlankLineWanted::Preserve);
                }
                self.mark_for_partial_format();
                self.visit_enum_constant(ec);
                first = false;
            }
            if self.b.peek_is(",") {
                self.token(",");
                self.b.forced_break();
            }
            self.b.close();
            self.b.close();
            if self.b.peek_is(";") {
                self.b.open(self.plus_two.clone());
                self.token(";");
                self.b.forced_break();
                self.drop_empty_declarations();
                self.b.close();
            }
            self.b.open(ZERO);
            self.add_body_declarations(&c.members, false, false);
            self.b.forced_break();
            self.b.blank_line_wanted(BlankLineWanted::No);
            self.token_indent("}", self.plus_two.clone());
            self.b.close();
        }
        self.b.guess_token(";");
    }

    fn visit_enum_constant(&mut self, ec: &EnumConstant) {
        for a in &ec.mods.annotations {
            self.scan(a);
            self.b.forced_break();
        }
        self.token(&ec.name);
        if ec.args.is_empty() {
            self.b.guess_token("(");
            self.b.guess_token(")");
        } else {
            self.add_arguments(&ec.args, self.plus_four.clone());
        }
        if let Some(body) = &ec.body {
            self.add_body_declarations(body, true, true);
        }
    }

    fn class_declaration_type_list(&mut self, keyword: &str, types: &[Tree]) {
        if types.is_empty() {
            return;
        }
        self.b.break_to_fill(" ");
        self.b.open(if types.len() > 1 { self.plus_four.clone() } else { ZERO });
        self.token(keyword);
        self.b.space();
        let mut first = true;
        for t in types {
            if !first {
                self.token(",");
                self.b.break_flat(" ");
            }
            self.scan(t);
            first = false;
        }
        self.b.close();
    }

    fn type_parameters_rest(&mut self, type_params: &[Tree], plus_indent: Indent) {
        self.b.open(plus_indent);
        self.b.break_op();
        self.b.open(ZERO);
        let mut first = true;
        for tp in type_params {
            if !first {
                self.token(",");
                self.b.break_flat(" ");
            }
            self.scan(tp);
            first = false;
        }
        self.token(">");
        self.b.close();
        self.b.close();
    }

    fn has_java_doc(&self, t: &Tree) -> bool {
        let input = self.b.input;
        if let Some(token) = input.token_at(t.start) {
            for &tb in &input.tokens[token].before {
                if input.tok(tb).text.starts_with("/**") {
                    return true;
                }
            }
        }
        false
    }

    fn add_body_declarations(&mut self, decls: &[Tree], braces: bool, first0: bool) {
        if decls.is_empty() {
            if braces {
                self.b.space();
                self.token_break_trailing_comment("{", self.plus_two.clone());
                self.b.blank_line_wanted(BlankLineWanted::No);
                self.b.open(ZERO);
                self.token_indent("}", self.plus_two.clone());
                self.b.close();
            }
            return;
        }
        if braces {
            self.b.space();
            self.token_break_trailing_comment("{", self.plus_two.clone());
            self.b.open_op(OpenOp::new(ZERO).breakability(LastLevelBreakability::AcceptInlineChain));
        }
        self.b.open(self.plus_two.clone());
        let mut first = first0;
        let mut last_one_got_blank_line_before = false;
        for decl in decls {
            self.drop_empty_declarations();
            self.b.forced_break();
            let is_var = matches!(decl.kind, K::Vars(_));
            let this_one_gets_blank_line_before = !is_var || self.has_java_doc(decl);
            if first {
                self.b.blank_line_wanted(BlankLineWanted::Preserve);
            } else if this_one_gets_blank_line_before || last_one_got_blank_line_before {
                self.b.blank_line_wanted(BlankLineWanted::Yes);
            }
            self.mark_for_partial_format();
            if let K::Vars(vars) = &decl.kind {
                self.sync(decl.start);
                let refs: Vec<&VarDecl> = vars.iter().collect();
                self.visit_variables(&refs, DeclKind::Field, Direction::Vertical);
            } else {
                self.scan(decl);
            }
            first = false;
            last_one_got_blank_line_before = this_one_gets_blank_line_before;
        }
        self.drop_empty_declarations();
        self.b.forced_break();
        self.b.close();
        self.b.forced_break();
        self.mark_for_partial_format();
        if braces {
            self.b.blank_line_wanted(BlankLineWanted::No);
            self.token_indent("}", self.plus_two.clone());
            self.b.close();
        }
    }

    // ------------------------------------------------------------------------------------------------------------
    // Methods
    // ------------------------------------------------------------------------------------------------------------

    fn visit_method(&mut self, m: &MethodDecl) {
        let is_record_constructor = m.compact_ctor;
        let ops = self.visit_modifiers(&m.mods.annotations, Direction::Vertical, None, false);
        self.b.add_all(ops);
        let (base_return_type, mut dims) = match &m.ret {
            Some(r) => {
                let (base, dims) = extract_dims(r);
                (Some(base), Some(VecDeque::from(dims)))
            }
            None => (None, None),
        };
        self.b.open(self.plus_four.clone());
        let break_before_name = self.b.new_tag();
        let break_before_type = self.b.new_tag();
        self.b.open(ZERO);
        {
            let mut first = true;
            if !m.type_params.is_empty() {
                self.token("<");
                self.type_parameters_rest(&m.type_params, self.plus_four.clone());
                if !m.return_type_annotations.is_empty() {
                    self.b.break_to_fill(" ");
                    self.visit_annotations(&m.return_type_annotations, false, false);
                }
                first = false;
            }
            let mut opened_name_and_type_scope = false;
            if let Some(base) = base_return_type {
                if !first {
                    self.b.break_full(FillMode::Independent, " ", ZERO, Some(break_before_type));
                } else {
                    first = false;
                }
                if !opened_name_and_type_scope {
                    self.b.open(Indent::if_(break_before_type, self.plus_four.clone(), ZERO));
                    opened_name_and_type_scope = true;
                }
                self.scan(base);
                if let Some(d) = dims.as_mut() {
                    self.maybe_add_dims(&mut VecDeque::new(), d);
                }
            }
            if !first {
                self.b.break_full(FillMode::Independent, " ", ZERO, Some(break_before_name));
            }
            if !opened_name_and_type_scope {
                self.b.open(ZERO);
            }
            self.token(&m.name);
            if !is_record_constructor {
                self.token("(");
            }
            self.b.close();
        }
        self.b.close();
        self.b.open(Indent::if_(break_before_name, self.plus_four.clone(), ZERO));
        self.b.open(Indent::if_(break_before_type, self.plus_four.clone(), ZERO));
        self.b.open(ZERO);
        {
            if !is_record_constructor {
                if !m.params.is_empty() || m.receiver.is_some() {
                    self.b.break_to_fill("");
                    let params: Vec<&VarDecl> = m.params.iter().collect();
                    self.visit_formals(m.receiver.as_deref(), &params);
                }
                self.token(")");
            }
            if let Some(d) = dims.as_mut() {
                self.maybe_add_dims(&mut VecDeque::new(), d);
            }
            if !m.throws.is_empty() {
                self.b.break_to_fill(" ");
                self.b.open(self.plus_four.clone());
                self.token("throws");
                self.b.break_to_fill(" ");
                let mut first = true;
                for t in &m.throws {
                    if !first {
                        self.token(",");
                        self.b.break_to_fill(" ");
                    }
                    self.scan(t);
                    first = false;
                }
                self.b.close();
            }
            if let Some(dv) = &m.default_value {
                self.b.space();
                self.token("default");
                if matches!(dv.kind, K::NewArray { .. }) {
                    self.b.open(self.minus_four.clone());
                    self.b.space();
                    self.scan(dv);
                    self.b.close();
                } else {
                    self.b.open(ZERO);
                    self.b.break_to_fill(" ");
                    self.scan(dv);
                    self.b.close();
                }
            }
        }
        self.b.close();
        self.b.close();
        self.b.close();
        match &m.body {
            None => self.token(";"),
            Some(_) => {
                self.b.space();
                self.b.token("{", RealOrImaginary::Real, self.plus_two.clone(), Some(self.plus_two.clone()));
            }
        }
        self.b.close();
        if let Some(body) = &m.body {
            self.sync(body.start);
            let K::Block { stmts, .. } = &body.kind else { unreachable!() };
            if stmts.is_empty() {
                self.b.blank_line_wanted(BlankLineWanted::No);
            } else {
                self.b.open(self.plus_two.clone());
                self.b.forced_break();
                self.b.blank_line_wanted(BlankLineWanted::Preserve);
                self.visit_statements(stmts, false);
                self.b.close();
                self.b.forced_break();
                self.b.blank_line_wanted(BlankLineWanted::No);
                self.mark_for_partial_format();
            }
            self.token_indent("}", self.plus_two.clone());
        }
    }

    fn visit_formals(&mut self, receiver: Option<&VarDecl>, params: &[&VarDecl]) {
        if receiver.is_none() && params.is_empty() {
            return;
        }
        let is_record_params = params.first().is_some_and(|p| p.is_record_component);
        self.b.open(ZERO);
        let mut first = true;
        if let Some(r) = receiver {
            self.declare_one(
                DeclKind::Parameter,
                Direction::Horizontal,
                Some((&r.mods, false)),
                r.ty.as_deref(),
                &r.name,
                "",
                "",
                None,
                if params.is_empty() { None } else { Some(",") },
                true,
                None,
                None,
            );
            first = false;
        }
        for (i, p) in params.iter().enumerate() {
            if !first {
                self.b.break_flat(" ");
            }
            let declaration_annotation_break = self.b.new_tag();
            if is_record_params && !first {
                self.b.blank_line_wanted(BlankLineWanted::Conditional(vec![declaration_annotation_break]));
            }
            self.visit_to_declare(
                DeclKind::Parameter,
                Direction::Horizontal,
                p,
                None,
                "=",
                if i < params.len() - 1 { Some(",") } else { None },
                Some(declaration_annotation_break),
            );
            if is_record_params && i < params.len() - 1 {
                self.b.blank_line_wanted(BlankLineWanted::Conditional(vec![declaration_annotation_break]));
            }
            first = false;
        }
        self.b.close();
    }

    // ------------------------------------------------------------------------------------------------------------
    // Modifiers and annotations
    // ------------------------------------------------------------------------------------------------------------

    fn next_is_modifier(&self) -> bool {
        matches!(
            self.b.peek_token(),
            Some(
                "public"
                    | "protected"
                    | "private"
                    | "abstract"
                    | "static"
                    | "final"
                    | "transient"
                    | "volatile"
                    | "synchronized"
                    | "native"
                    | "strictfp"
                    | "default"
                    | "sealed"
                    | "non"
                    | "-"
            )
        )
    }

    fn visit_and_break_modifiers(&mut self, mods: &Mods, is_record: bool, direction: Direction, tag: Option<BreakTag>) {
        let ops = self.visit_modifiers(&mods.annotations, direction, tag, is_record);
        self.b.add_all(ops);
    }

    fn visit_modifiers(
        &mut self,
        annotation_trees: &[Tree],
        direction: Direction,
        tag: Option<BreakTag>,
        is_record_parameter: bool,
    ) -> Vec<Op> {
        if annotation_trees.is_empty() && !self.next_is_modifier() {
            return Vec::new();
        }
        let mut annotations: VecDeque<&Tree> = annotation_trees.iter().collect();
        self.b.open(ZERO);
        let mut first = true;
        let mut last_was_annotation = false;
        while !annotations.is_empty() {
            if self.next_is_modifier() {
                break;
            }
            if !first {
                let ops = if direction == Direction::Vertical { force_break_list(tag) } else { break_list(tag) };
                self.b.add_all(ops);
            }
            let a = annotations.pop_front().unwrap();
            self.scan(a);
            first = false;
            last_was_annotation = true;
        }
        self.b.close();
        let trailing_break = if is_record_parameter {
            vec![Op::Break(BreakDoc {
                fill_mode: FillMode::Unified,
                flat: " ",
                plus_indent: ZERO,
                opt_tag: tag,
                has_column_limit: true,
            })]
        } else if direction == Direction::Vertical {
            force_break_list(tag)
        } else {
            break_list(tag)
        };
        if annotations.is_empty() && !self.next_is_modifier() {
            return trailing_break;
        }
        if last_was_annotation {
            self.b.add_all(trailing_break);
        }
        self.b.open(ZERO);
        first = true;
        while self.next_is_modifier() || !annotations.is_empty() {
            if !first {
                self.b.add_all(break_fill_list(None));
            }
            if self.next_is_modifier() {
                let current = self.b.peek_token().unwrap();
                self.token(current);
                if current == "non" {
                    let t = self.b.peek_token().unwrap();
                    self.token(t);
                    let t = self.b.peek_token().unwrap();
                    self.token(t);
                }
            } else {
                let a = annotations.pop_front().unwrap();
                self.scan(a);
            }
            first = false;
        }
        self.b.close();
        break_fill_list(None)
    }

    fn visit_annotations(&mut self, annotations: &[Tree], break_before: bool, break_after: bool) {
        if annotations.is_empty() {
            return;
        }
        if break_before {
            self.b.break_to_fill(" ");
        }
        let mut first = true;
        for a in annotations {
            if !first {
                self.b.break_to_fill(" ");
            }
            self.scan(a);
            first = false;
        }
        if break_after {
            self.b.break_to_fill(" ");
        }
    }

    fn visit_annotation(&mut self, t: &Tree) {
        let K::Annotation { ty, args } = &t.kind else { unreachable!() };
        self.sync(t.start);
        // visitSingleMemberAnnotation
        if args.len() == 1 && !matches!(args[0].kind, K::Assign { .. }) {
            let value = &args[0];
            let is_array_initializer = matches!(value.kind, K::NewArray { .. });
            self.b.open_op(
                OpenOp::new(if is_array_initializer { ZERO } else { self.plus_four.clone() })
                    .behaviour(prefer_breaking_last(true))
                    .breakability(LastLevelBreakability::CheckInner),
            );
            self.token("@");
            self.scan(ty);
            self.token("(");
            if !is_array_initializer {
                self.b.break_op();
            }
            self.scan(value);
            self.b.close();
            self.token(")");
            return;
        }
        self.b.open(ZERO);
        self.token("@");
        self.scan(ty);
        if !args.is_empty() {
            self.b.open(self.plus_four.clone());
            self.token("(");
            self.b.break_op();
            let has_array_initializer = args.iter().any(is_array_value);
            let mut first = true;
            for arg in args {
                if !first {
                    self.token(",");
                    if has_array_initializer {
                        self.b.forced_break();
                    } else {
                        self.b.break_flat(" ");
                    }
                }
                if let K::Assign { lhs, rhs, .. } = &arg.kind {
                    self.visit_annotation_argument(arg, lhs, rhs);
                } else {
                    self.scan(arg);
                }
                first = false;
            }
            self.token(")");
            self.b.close();
            self.b.close();
            return;
        } else if self.b.peek_is("(") {
            self.token("(");
            self.token(")");
        }
        self.b.close();
    }

    fn visit_annotation_argument(&mut self, t: &Tree, lhs: &Tree, rhs: &Tree) {
        let is_array_initializer = matches!(rhs.kind, K::NewArray { .. });
        self.sync(t.start);
        self.b.open_op(
            OpenOp::new(if is_array_initializer { ZERO } else { self.plus_four.clone() })
                .behaviour(prefer_breaking_last(true))
                .breakability(LastLevelBreakability::AcceptInlineChainIfSimpleOtherwiseCheckInner),
        );
        self.scan(lhs);
        self.b.space();
        self.token("=");
        if is_array_initializer {
            self.b.space();
        } else {
            self.b.break_flat(" ");
        }
        self.scan(rhs);
        self.b.close();
    }

    // ------------------------------------------------------------------------------------------------------------
    // Variables
    // ------------------------------------------------------------------------------------------------------------

    fn visit_variables(&mut self, fragments: &[&VarDecl], kind: DeclKind, direction: Direction) {
        if fragments.len() == 1 {
            let f = fragments[0];
            let type_with_dims = f.ty.as_deref().map(|t| {
                let (base, dims) = extract_dims(t);
                (Some(base), dims)
            });
            self.declare_one(
                kind,
                direction,
                Some((&f.mods, f.is_record_component)),
                f.ty.as_deref(),
                &f.name,
                "",
                "=",
                f.init.as_deref(),
                Some(";"),
                false,
                type_with_dims,
                None,
            );
        } else {
            self.declare_many(fragments, direction);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn visit_to_declare(
        &mut self,
        kind: DeclKind,
        direction: Direction,
        node: &VarDecl,
        initializer: Option<&Tree>,
        equals: &str,
        trailing: Option<&str>,
        annotation_break_for_records: Option<BreakTag>,
    ) {
        self.sync(node.start);
        self.declare_one(
            kind,
            direction,
            Some((&node.mods, node.is_record_component)),
            node.ty.as_deref(),
            &node.name,
            "",
            equals,
            initializer,
            trailing,
            false,
            None,
            annotation_break_for_records,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn declare_one(
        &mut self,
        kind: DeclKind,
        direction: Direction,
        modifiers: Option<(&Mods, bool)>,
        ty: Option<&Tree>,
        name: &str,
        op: &str,
        equals: &str,
        initializer: Option<&Tree>,
        trailing: Option<&str>,
        name_is_receiver_expression: bool,
        type_with_dims: Option<(Option<&Tree>, Vec<&Vec<Tree>>)>,
        vertical_annotation_break_for_records: Option<BreakTag>,
    ) -> usize {
        let type_break = self.b.new_tag();
        let vertical_annotation_break = vertical_annotation_break_for_records.unwrap_or_else(|| self.b.new_tag());
        let is_field = kind == DeclKind::Field;
        if is_field {
            self.b.blank_line_wanted(BlankLineWanted::Conditional(vec![vertical_annotation_break]));
        }
        let mut dims: VecDeque<&Vec<Tree>> =
            type_with_dims.as_ref().map(|(_, d)| d.iter().copied().collect()).unwrap_or_default();
        let mut base_dims = 0;
        match modifiers {
            Some((mods, is_record)) if kind == DeclKind::Parameter && !mods.annotations.is_empty() => {
                if !is_record {
                    self.b.open(self.plus_four.clone());
                } else {
                    self.b.open_op(
                        OpenOp::new(ZERO).name("visitParam").column_limit(ANNOTATION_RECORD_PARAMETER_COLUMN_LIMIT),
                    );
                }
            }
            _ => self.b.open(ZERO),
        }
        {
            if let Some((mods, is_record)) = modifiers {
                self.visit_and_break_modifiers(mods, is_record, direction, Some(vertical_annotation_break));
            }
            let is_var = self.b.peek_is("var") && (name != "var" || self.b.peek_token_skip(1) == Some("var"));
            let has_type = ty.is_some() || is_var;
            self.b.open(if has_type { self.plus_four.clone() } else { ZERO });
            {
                self.b.open(ZERO);
                {
                    self.b.open(ZERO);
                    if let Some((Some(node), _)) = &type_with_dims {
                        self.scan(node);
                        let total_dims = dims.len();
                        self.b.open(self.plus_four.clone());
                        self.maybe_add_dims(&mut VecDeque::new(), &mut dims);
                        self.b.close();
                        base_dims = total_dims - dims.len();
                    } else if is_var {
                        self.token("var");
                    } else if let Some(ty) = ty {
                        self.scan(ty);
                    }
                    self.b.close();
                    if has_type {
                        self.b.break_full(FillMode::Independent, " ", ZERO, Some(type_break));
                    }
                    self.b.open(Indent::if_(type_break, self.plus_four.clone(), ZERO));
                    if name_is_receiver_expression {
                        let mut first = true;
                        for part in name.split('.') {
                            if !first {
                                self.token(".");
                            }
                            self.token(part);
                            first = false;
                        }
                    } else {
                        self.token(name);
                    }
                    self.b.op(op);
                }
                self.maybe_add_dims(&mut VecDeque::new(), &mut dims);
                self.b.close();
            }
            self.b.close();
            if let Some(init) = initializer {
                self.b.space();
                self.token(equals);
                if matches!(init.kind, K::NewArray { elem: None, .. }) {
                    self.b.open(self.minus_four.clone());
                    self.b.space();
                    self.visit(init);
                    self.b.close();
                } else {
                    self.b.open_op(
                        OpenOp::new(Indent::if_(type_break, self.plus_four.clone(), ZERO))
                            .behaviour(break_only_if_inner(true))
                            .breakability(LastLevelBreakability::Abort),
                    );
                    self.b.break_to_fill(" ");
                    self.scan(init);
                    self.b.close();
                }
            }
            if let Some(tr) = trailing
                && self.b.peek_is(tr)
            {
                self.b.guess_token(tr);
            }
            self.b.close();
        }
        self.b.close();
        if is_field {
            self.b.blank_line_wanted(BlankLineWanted::Conditional(vec![vertical_annotation_break]));
        }
        base_dims
    }

    fn declare_many(&mut self, fragments: &[&VarDecl], direction: Direction) {
        self.b.open(ZERO);
        let first_fragment = fragments[0];
        self.visit_and_break_modifiers(&first_fragment.mods, false, direction, None);
        self.b.open(self.plus_four.clone());
        self.b.open(ZERO);
        let ty = first_fragment.ty.as_deref().expect("declare_many requires a type");
        let (base, extracted) = extract_dims(ty);
        let mut dims: VecDeque<&Vec<Tree>> = extracted.into_iter().collect();
        self.scan(base);
        let mut base_dims = dims.len();
        self.maybe_add_dims(&mut VecDeque::new(), &mut dims);
        base_dims -= dims.len();
        let mut first = true;
        for fragment in fragments {
            if !first {
                self.token(",");
            }
            let fragment_dims: Vec<&Vec<Tree>> = match fragment.ty.as_deref() {
                Some(t) => {
                    let (_, d) = extract_dims(t);
                    if first {
                        d
                    } else if base_dims > 0 {
                        let n = d.len().saturating_sub(base_dims);
                        d[..n].to_vec()
                    } else {
                        d
                    }
                }
                None => Vec::new(),
            };
            let mut dims: VecDeque<&Vec<Tree>> = fragment_dims.into_iter().collect();
            self.b.break_flat(" ");
            self.b.open(ZERO);
            self.maybe_add_dims(&mut VecDeque::new(), &mut dims);
            self.token(&fragment.name);
            self.maybe_add_dims(&mut VecDeque::new(), &mut dims);
            if let Some(init) = &fragment.init {
                self.b.space();
                self.token("=");
                self.b.open(self.plus_four.clone());
                self.b.break_flat(" ");
                self.scan(init);
                self.b.close();
            }
            self.b.close();
            if first {
                self.b.close();
            }
            first = false;
        }
        self.b.close();
        self.token(";");
        self.b.close();
    }

    /// Emits array dimensions, using look-ahead to preserve the original syntax.
    fn maybe_add_dims(&mut self, dim_expressions: &mut VecDeque<&Tree>, annotations: &mut VecDeque<&Vec<Tree>>) {
        let mut last_was_annotation = false;
        while let Some(tok) = self.b.peek_token() {
            match tok {
                "@" => {
                    let Some(dim_annotations) = annotations.pop_front() else {
                        return;
                    };
                    if dim_annotations.is_empty() {
                        continue;
                    }
                    self.b.break_to_fill(" ");
                    self.visit_annotations(dim_annotations, false, false);
                    last_was_annotation = true;
                }
                "[" => {
                    if last_was_annotation {
                        self.b.break_to_fill(" ");
                    } else {
                        self.b.break_to_fill("");
                    }
                    self.token("[");
                    if self.b.peek_token() != Some("]")
                        && let Some(e) = dim_expressions.pop_front()
                    {
                        self.scan(e);
                    }
                    self.token("]");
                    last_was_annotation = false;
                }
                "." => {
                    if self.b.peek_token_skip(1) != Some(".") || self.b.peek_token_skip(2) != Some(".") {
                        return;
                    }
                    if last_was_annotation {
                        self.b.break_to_fill(" ");
                    } else {
                        self.b.break_to_fill("");
                    }
                    self.b.op("...");
                    last_was_annotation = false;
                }
                _ => return,
            }
        }
    }

    fn visit_annotated_array_type(&mut self, t: &Tree) {
        let (base, dims) = extract_dims(t);
        self.b.open(self.plus_four.clone());
        self.scan(base);
        let mut dims: VecDeque<&Vec<Tree>> = dims.into_iter().collect();
        self.maybe_add_dims(&mut VecDeque::new(), &mut dims);
        self.b.close();
    }

    // ------------------------------------------------------------------------------------------------------------
    // Statements
    // ------------------------------------------------------------------------------------------------------------

    fn visit_block(&mut self, t: &Tree, collapse_empty: bool, allow_leading_blank: bool, allow_trailing_blank: bool) {
        let K::Block { is_static, stmts } = &t.kind else { unreachable!() };
        self.sync(t.start);
        if *is_static {
            self.token("static");
            self.b.space();
        }
        if collapse_empty && stmts.is_empty() {
            if self.b.peek_is(";") {
                self.token(";");
            } else {
                self.token_break_trailing_comment("{", self.plus_two.clone());
                self.b.blank_line_wanted(BlankLineWanted::No);
                self.token_indent("}", self.plus_two.clone());
            }
        } else {
            self.b.open(ZERO);
            self.b.open(self.plus_two.clone());
            self.token_break_trailing_comment("{", self.plus_two.clone());
            self.b.blank_line_wanted(if allow_leading_blank { BlankLineWanted::Preserve } else { BlankLineWanted::No });
            self.visit_statements(stmts, false);
            self.b.close();
            self.b.forced_break();
            self.b.close();
            self.b.blank_line_wanted(if allow_trailing_blank {
                BlankLineWanted::Preserve
            } else {
                BlankLineWanted::No
            });
            self.mark_for_partial_format();
            self.token_indent("}", self.plus_two.clone());
        }
    }

    fn visit_statement(&mut self, t: &Tree, collapse_empty: bool, allow_leading_blank: bool, allow_trailing: bool) {
        self.sync(t.start);
        if is_block(t) {
            self.b.space();
            self.visit_block(t, collapse_empty, allow_leading_blank, allow_trailing);
        } else {
            self.b.open(self.plus_two.clone());
            self.b.break_flat(" ");
            self.scan(t);
            self.b.close();
        }
    }

    fn visit_statements(&mut self, stmts: &[Tree], inline_first: bool) {
        let mut first = true;
        self.drop_empty_declarations();
        for s in stmts {
            if !(inline_first && first) {
                self.b.forced_break();
            }
            if !first {
                self.b.blank_line_wanted(BlankLineWanted::Preserve);
            }
            self.mark_for_partial_format();
            first = false;
            if let K::Vars(vars) = &s.kind {
                self.sync(s.start);
                let refs: Vec<&VarDecl> = vars.iter().collect();
                self.visit_variables(&refs, DeclKind::None, can_local_have_horizontal_annotations(&vars[0].mods));
            } else {
                self.scan(s);
            }
        }
    }

    fn visit_if(&mut self, t: &Tree) {
        self.sync(t.start);
        let mut expressions = Vec::new();
        let mut statements = Vec::new();
        let mut node = t;
        let final_else;
        loop {
            let K::If { cond, then, else_ } = &node.kind else { unreachable!() };
            expressions.push(&**cond);
            statements.push(&**then);
            match else_ {
                Some(e) if matches!(e.kind, K::If { .. }) => node = e,
                other => {
                    final_else = other.as_deref();
                    break;
                }
            }
        }
        self.b.open(ZERO);
        let mut first = true;
        let mut following_block = false;
        let n = expressions.len();
        for i in 0..n {
            if !first {
                if following_block {
                    self.b.space();
                } else {
                    self.b.forced_break();
                }
                self.token("else");
                self.b.space();
            }
            self.token("if");
            self.b.space();
            self.token("(");
            self.scan(skip_paren(expressions[i]));
            self.token(")");
            let only_clause = n == 1 && final_else.is_none();
            let trailing_clauses = i < n - 1 || final_else.is_some();
            self.visit_statement(statements[i], only_clause, true, trailing_clauses);
            following_block = is_block(statements[i]);
            first = false;
        }
        if let Some(e) = final_else {
            if following_block {
                self.b.space();
            } else {
                self.b.forced_break();
            }
            self.token("else");
            self.visit_statement(e, false, true, false);
        }
        self.b.close();
    }

    fn visit_for(&mut self, t: &Tree, init: &[Tree], cond: Option<&Tree>, update: &[Tree], body: &Tree) {
        self.sync(t.start);
        self.token("for");
        self.b.space();
        self.token("(");
        self.b.open(self.plus_four.clone());
        let expr_inits = init.iter().filter(|i| matches!(i.kind, K::ExprStmt(_))).count();
        self.b.open(if expr_inits > 1 { self.plus_four.clone() } else { ZERO });
        if !init.is_empty() {
            if let K::Vars(vars) = &init[0].kind {
                let refs: Vec<&VarDecl> = vars.iter().collect();
                self.visit_variables(&refs, DeclKind::None, Direction::Horizontal);
            } else {
                let mut first = true;
                self.b.open(ZERO);
                for i in init {
                    if !first {
                        self.token(",");
                        self.b.break_flat(" ");
                    }
                    if let K::ExprStmt(e) = &i.kind {
                        self.scan(e);
                    }
                    first = false;
                }
                self.token(";");
                self.b.close();
            }
        } else {
            self.token(";");
        }
        self.b.close();
        self.b.break_flat(" ");
        if let Some(c) = cond {
            self.scan(c);
        }
        self.token(";");
        if !update.is_empty() {
            self.b.break_flat(" ");
            self.b.open(if update.len() <= 1 { ZERO } else { self.plus_four.clone() });
            let mut first = true;
            for u in update {
                if !first {
                    self.token(",");
                    self.b.break_to_fill(" ");
                }
                self.scan(u);
                first = false;
            }
            self.b.guess_token(";");
            self.b.close();
        } else {
            self.b.space();
        }
        self.b.close();
        self.token(")");
        self.visit_statement(body, true, true, false);
    }

    fn visit_switch(&mut self, selector: &Tree, cases: &[Case]) {
        self.b.open(ZERO);
        self.token("switch");
        self.b.space();
        self.token("(");
        self.scan(skip_paren(selector));
        self.token(")");
        self.b.space();
        self.token_break_trailing_comment("{", self.plus_two.clone());
        self.b.blank_line_wanted(BlankLineWanted::No);
        self.b.open(self.plus_two.clone());
        let mut after_first = false;
        for c in cases {
            if after_first {
                self.b.blank_line_wanted(BlankLineWanted::Preserve);
            }
            self.visit_case(c);
            after_first = true;
        }
        self.b.close();
        self.b.forced_break();
        self.b.blank_line_wanted(BlankLineWanted::No);
        self.token_indent("}", self.plus_two.clone());
        self.b.close();
    }

    fn visit_case(&mut self, c: &Case) {
        self.sync(c.start);
        self.mark_for_partial_format();
        self.b.forced_break();
        if c.is_default {
            self.token("default");
        } else {
            self.token("case");
            self.b.open(if c.labels.len() > 1 { self.plus_four.clone() } else { ZERO });
            self.b.space();
            let mut after_first = false;
            for l in &c.labels {
                if after_first {
                    self.token(",");
                    self.b.break_flat(" ");
                }
                self.scan(l);
                after_first = true;
            }
            self.b.close();
        }
        if let Some(g) = &c.guard {
            self.b.break_to_fill(" ");
            self.token("when");
            self.b.space();
            self.scan(g);
        }
        match c.kind {
            CaseKind::Statement => {
                self.token(":");
                let is_blk = c.stmts.len() == 1 && is_block(&c.stmts[0]);
                self.b.open(if is_blk { ZERO } else { self.plus_two.clone() });
                if is_blk {
                    self.b.space();
                }
                self.visit_statements(&c.stmts, is_blk);
                self.b.close();
            }
            CaseKind::Rule => {
                self.b.open(self.plus_two.clone());
                self.b.space();
                self.token("-");
                self.token(">");
                let body = c.body.as_deref().unwrap();
                if is_block(body) {
                    self.b.close();
                    self.b.space();
                    self.visit_block(body, true, false, false);
                } else {
                    self.b.break_flat(" ");
                    self.scan(body);
                    self.b.close();
                }
                self.b.guess_token(";");
            }
        }
    }

    fn visit_try(&mut self, t: &Tree, resources: &[Tree], block: &Tree, catches: &[Catch], finally: Option<&Tree>) {
        self.sync(t.start);
        self.b.open(ZERO);
        self.token("try");
        self.b.space();
        if !resources.is_empty() {
            self.token("(");
            self.b.open(if resources.len() > 1 { self.plus_four.clone() } else { ZERO });
            let mut first = true;
            for r in resources {
                if !first {
                    self.b.forced_break();
                }
                if let K::Var(v) = &r.kind {
                    self.declare_one(
                        DeclKind::Parameter,
                        inline_annotation_direction(&v.mods),
                        Some((&v.mods, false)),
                        v.ty.as_deref(),
                        &v.name,
                        "",
                        "=",
                        v.init.as_deref(),
                        None,
                        false,
                        None,
                        None,
                    );
                } else {
                    self.scan(r);
                }
                if self.b.peek_is(";") {
                    self.token(";");
                    self.b.space();
                }
                first = false;
            }
            if self.b.peek_is(";") {
                self.token(";");
                self.b.space();
            }
            self.token(")");
            self.b.close();
            self.b.space();
        }
        let trailing_clauses = !catches.is_empty() || finally.is_some();
        self.visit_block(block, !trailing_clauses, true, trailing_clauses);
        for (i, c) in catches.iter().enumerate() {
            let trailing = i < catches.len() - 1 || finally.is_some();
            self.visit_catch_clause(c, trailing);
        }
        if let Some(f) = finally {
            self.b.space();
            self.token("finally");
            self.b.space();
            self.visit_block(f, false, true, false);
        }
        self.b.close();
    }

    fn visit_catch_clause(&mut self, c: &Catch, allow_trailing_blank: bool) {
        self.sync(c.start);
        self.b.space();
        self.token("catch");
        self.b.space();
        self.token("(");
        self.b.open(self.plus_four.clone());
        let ex = &c.param;
        if !c.union.is_empty() {
            self.b.open(ZERO);
            // visitUnionType
            self.b.open(ZERO);
            self.sync(ex.start);
            self.visit_and_break_modifiers(&ex.mods, false, Direction::Horizontal, None);
            let n = c.union.len();
            let mut first = true;
            for alt in &c.union[..n - 1] {
                if !first {
                    self.b.break_flat(" ");
                    self.token("|");
                    self.b.space();
                } else {
                    first = false;
                }
                self.scan(alt);
            }
            self.b.break_flat(" ");
            self.token("|");
            self.b.space();
            self.declare_one(
                DeclKind::None,
                Direction::Horizontal,
                None,
                Some(&c.union[n - 1]),
                &ex.name,
                "",
                "=",
                None,
                None,
                false,
                None,
                None,
            );
            self.b.close();
            self.b.close();
        } else {
            self.b.break_to_fill("");
            self.b.open(ZERO);
            self.sync(ex.start);
            self.visit_variables(&[ex], DeclKind::None, inline_annotation_direction(&ex.mods));
            self.b.close();
        }
        self.b.close();
        self.token(")");
        self.b.space();
        self.visit_block(&c.block, false, true, allow_trailing_blank);
    }

    // ------------------------------------------------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------------------------------------------------

    fn visit_binary(&mut self, t: &Tree) {
        self.sync(t.start);
        let K::Binary { op, .. } = &t.kind else { unreachable!() };
        let mut operands = Vec::new();
        let mut operators = Vec::new();
        walk_infix(precedence(op), t, &mut operands, &mut operators);
        let is_string_concat = is_string_concat(t);
        let should_preserve_newlines = is_string_concat && self.line_span(t) > 2;
        let fill_mode = if self.has_only_short_items(&operands) || is_string_concat {
            FillMode::Independent
        } else {
            FillMode::Unified
        };
        self.b.open_op(
            OpenOp::new(self.plus_four.clone())
                .breakability(LastLevelBreakability::AcceptInlineChainIfSimpleOtherwiseCheckInner),
        );
        self.scan(operands[0]);
        let mut should_enforce_newline = self.b.most_recent_token_followed_by_newline();
        for i in 0..operators.len() {
            let next_fill_mode =
                if should_preserve_newlines && should_enforce_newline { FillMode::Unified } else { fill_mode };
            self.b.break_full(next_fill_mode, " ", ZERO, None);
            self.b.op(operators[i]);
            should_enforce_newline = self.b.most_recent_token_followed_by_newline();
            self.b.space();
            self.scan(operands[i + 1]);
            should_enforce_newline = should_enforce_newline || self.b.most_recent_token_followed_by_newline();
        }
        self.b.close();
    }

    fn line_span(&self, t: &Tree) -> usize {
        let input = self.b.input;
        let start = t.start;
        let mut end = t.end;
        while end > start && input.token_at(end).is_none() {
            end -= 1;
        }
        let (Some(st), Some(et)) = (input.token_at(start), input.token_at(end)) else {
            return 1;
        };
        input.line_number(input.token_tok(et).position) - input.line_number(input.token_tok(st).position) + 1
    }

    fn has_only_short_items(&self, expressions: &[&Tree]) -> bool {
        expressions.iter().all(|e| self.b.actual_size(e.start, e.end - e.start) < MAX_ITEM_LENGTH_FOR_FILLING)
    }

    fn visit_new_class(
        &mut self,
        t: &Tree,
        outer: Option<&Tree>,
        type_args: &[Tree],
        ident: &Tree,
        args: &[Tree],
        body: Option<&[Tree]>,
    ) {
        self.sync(t.start);
        let breakability = if body.is_some() {
            LastLevelBreakability::AcceptInlineChain
        } else {
            LastLevelBreakability::AcceptInlineChainIfSimpleOtherwiseCheckInner
        };
        self.b.open_op(
            OpenOp::new(ZERO).name("visitNewClass").behaviour(prefer_breaking_last(true)).breakability(breakability),
        );
        if let Some(o) = outer {
            self.scan(o);
            self.b.break_op();
            self.token(".");
        }
        self.token("new");
        self.b.space();
        self.add_type_arguments(type_args, self.plus_four.clone());
        if body.is_some() {
            let ops = self.visit_modifiers(&[], Direction::Horizontal, None, false);
            self.b.add_all(ops);
        }
        self.scan(ident);
        self.add_arguments(args, self.plus_four.clone());
        self.b.close();
        if let Some(body) = body {
            self.add_body_declarations(body, true, true);
        }
    }

    fn visit_new_array(
        &mut self,
        elem: Option<&Tree>,
        dims: &[Tree],
        dim_annotations: &[Vec<Tree>],
        init: Option<&[Tree]>,
    ) {
        if let Some(elem) = elem {
            self.b.open_op(OpenOp::new(self.plus_four.clone()).breakability(LastLevelBreakability::CheckInner));
            self.token("new");
            self.b.space();
            let (base, extracted) = extract_dims(elem);
            let mut dim_expressions: VecDeque<&Tree> = dims.iter().collect();
            let empty = Vec::new();
            let mut annotations: VecDeque<&Vec<Tree>> = VecDeque::new();
            annotations.push_back(&empty);
            annotations.extend(dim_annotations.iter());
            annotations.extend(extracted);
            self.scan(base);
            self.b.open(ZERO);
            self.maybe_add_dims(&mut dim_expressions, &mut annotations);
            self.b.close();
            self.b.close();
        }
        if let Some(init) = init {
            if elem.is_some() {
                self.b.space();
            }
            self.visit_array_initializer(init);
        }
    }

    fn visit_array_initializer(&mut self, expressions: &[Tree]) {
        if expressions.is_empty() {
            self.token_break_trailing_comment("{", self.plus_two.clone());
            if self.b.peek_is(",") {
                self.token(",");
            }
            self.token_indent("}", self.plus_two.clone());
            return;
        }
        let exprs: Vec<&Tree> = expressions.iter().collect();
        let cols = self.arguments_are_tabular(&exprs);
        if cols != -1 {
            let cols = cols as usize;
            self.b.open_op(OpenOp::new(ZERO).breakability(LastLevelBreakability::AcceptInlineChain));
            self.b.open_op(OpenOp::new(self.plus_two.clone()).breakability(LastLevelBreakability::AcceptInlineChain));
            self.token("{");
            self.b.forced_break();
            let mut first = true;
            for row in exprs.chunks(cols) {
                if !first {
                    self.b.forced_break();
                }
                self.b.open(if matches!(row[0].kind, K::NewArray { .. }) || cols == 1 {
                    ZERO
                } else {
                    self.plus_four.clone()
                });
                let mut first_in_row = true;
                for item in row {
                    if !first_in_row {
                        self.token(",");
                        self.b.break_to_fill(" ");
                    }
                    self.scan(item);
                    first_in_row = false;
                }
                self.b.guess_token(",");
                self.b.close();
                first = false;
            }
            self.b.close();
            self.b.break_op();
            self.token_indent("}", self.plus_two.clone());
            self.b.close();
        } else {
            let n = self.path.len();
            let in_member_value_pair = (n >= 1 && self.path[n - 1]) || (n >= 2 && self.path[n - 2]);
            let short_items = self.has_only_short_items(&exprs);
            let allow_filled_elements_on_own_line = short_items || !in_member_value_pair;
            self.b.open_op(OpenOp::new(ZERO).breakability(LastLevelBreakability::AcceptInlineChain));
            self.b.open_op(OpenOp::new(self.plus_two.clone()).breakability(LastLevelBreakability::AcceptInlineChain));
            self.token_break_trailing_comment("{", self.plus_two.clone());
            let has_trailing_comma = self.has_trailing_token(exprs[exprs.len() - 1], ",");
            self.b.break_full(if has_trailing_comma { FillMode::Forced } else { FillMode::Unified }, "", ZERO, None);
            if allow_filled_elements_on_own_line {
                self.b.open(ZERO);
            }
            let mut first = true;
            let fill_mode = if short_items { FillMode::Independent } else { FillMode::Unified };
            for e in &exprs {
                if !first {
                    self.token(",");
                    self.b.break_full(fill_mode, " ", ZERO, None);
                }
                self.scan(e);
                first = false;
            }
            self.b.guess_token(",");
            if allow_filled_elements_on_own_line {
                self.b.close();
            }
            self.b.close();
            self.b.break_op();
            self.token_indent("}", self.plus_two.clone());
            self.b.close();
        }
    }

    fn has_trailing_token(&self, last: &Tree, token: &str) -> bool {
        let input = self.b.input;
        input.token_at(last.end).is_some_and(|t| input.token_tok(t).text == token)
    }

    fn actual_column(&self, e: &Tree) -> i64 {
        let pos = self.b.actual_start_column(e.start);
        self.b.input.column_at(pos).map(|c| c as i64).unwrap_or(-1)
    }

    /// Returns the number of columns if the arguments are laid out in a grid, or else -1.
    fn arguments_are_tabular(&self, arguments: &[&Tree]) -> i32 {
        if arguments.is_empty() {
            return -1;
        }
        let mut rows: Vec<Vec<&Tree>> = Vec::new();
        let mut it = arguments.iter().peekable();
        let start0 = self.actual_column(it.peek().unwrap());
        {
            let mut row = vec![*it.next().unwrap()];
            while let Some(next) = it.peek() {
                if self.actual_column(next) > start0 {
                    row.push(it.next().unwrap());
                } else {
                    break;
                }
            }
            if it.peek().is_none() {
                return -1;
            }
            if row_length(&row) <= 1 {
                return -1;
            }
            rows.push(row);
        }
        while let Some(next) = it.peek() {
            let start = self.actual_column(next);
            if start != start0 {
                return -1;
            }
            let mut row = vec![*it.next().unwrap()];
            while let Some(next) = it.peek() {
                if self.actual_column(next) > start0 {
                    row.push(it.next().unwrap());
                } else {
                    break;
                }
            }
            rows.push(row);
        }
        let size0 = rows[0].len();
        if !expressions_are_parallel(&rows, 0, rows.len()) {
            return -1;
        }
        for i in 1..size0 {
            if !expressions_are_parallel(&rows, i, rows.len() / 2 + 1) {
                return -1;
            }
        }
        if rows.len() == 2 {
            return if size0 == rows[1].len() { size0 as i32 } else { -1 };
        }
        for row in &rows[1..rows.len() - 1] {
            if size0 != row.len() {
                return -1;
            }
        }
        if size0 < rows[rows.len() - 1].len() {
            return -1;
        }
        size0 as i32
    }

    fn visit_lambda(&mut self, params: &[VarDecl], body: &Tree) {
        let statement_body = is_block(body) || matches!(body.kind, K::SwitchExpr { .. });
        let parens = self.b.peek_is("(");
        self.b.open_op(OpenOp::new(if parens { self.plus_four.clone() } else { ZERO }).name("lambda arguments"));
        if parens {
            self.token("(");
        }
        let mut first = true;
        for p in params {
            if !first {
                self.token(",");
                self.b.break_flat(" ");
            }
            self.sync(p.start);
            self.visit_variables(&[p], DeclKind::None, inline_annotation_direction(&p.mods));
            first = false;
        }
        if parens {
            self.token(")");
        }
        self.b.close();
        self.b.space();
        self.b.op("->");
        self.b.open_op(
            OpenOp::new(if statement_body { ZERO } else { self.plus_four.clone() })
                .name("lambda body")
                .behaviour(if statement_body { prefer_breaking_last(true) } else { break_only_if_inner(false) })
                .breakability(if statement_body {
                    LastLevelBreakability::AcceptInlineChain
                } else {
                    LastLevelBreakability::AcceptInlineChainIfSimpleOtherwiseCheckInner
                }),
        );
        if statement_body {
            self.b.space();
        } else {
            self.b.break_flat(" ");
        }
        if is_block(body) {
            self.visit_block(body, true, false, false);
        } else {
            self.scan(body);
        }
        self.b.close();
    }

    // ------------------------------------------------------------------------------------------------------------
    // Dot chains and method invocations
    // ------------------------------------------------------------------------------------------------------------

    fn handle_log_statement(&mut self, t: &Tree) -> bool {
        let K::MethodInvocation { select, .. } = &t.kind else { return false };
        if method_name(select) != "log" {
            return false;
        }
        let mut parts: VecDeque<&Tree> = VecDeque::new();
        let mut curr = t;
        while let K::MethodInvocation { select, .. } = &curr.kind {
            parts.push_front(curr);
            if !LOG_METHODS.contains(&method_name(select)) {
                return false;
            }
            match method_receiver(select) {
                Some(r) => curr = r,
                None => return false,
            }
        }
        if !matches!(curr.kind, K::Ident(_)) {
            return false;
        }
        parts.push_front(curr);
        let items: Vec<&Tree> = parts.into_iter().collect();
        let n = items.len();
        self.visit_dot_with_prefix(&items, false, &[n - 1], FillMode::Independent);
        true
    }

    fn visit_dot(&mut self, node0: &Tree) {
        let mut node = Some(node0);
        let mut stack: VecDeque<&Tree> = VecDeque::new();
        loop {
            let mut n = node.unwrap();
            stack.push_front(n);
            if matches!(n.kind, K::ArrayAccess { .. }) {
                n = array_base(n);
            }
            match &n.kind {
                K::MemberSelect { expr, .. } => node = Some(expr),
                K::MethodInvocation { select, .. } => node = method_receiver(select),
                K::Ident(_) => {
                    node = None;
                    break;
                }
                _ => {
                    node = stack.pop_front();
                    break;
                }
            }
            if node.is_none() {
                break;
            }
        }
        let items: Vec<&Tree> = stack.iter().copied().collect();
        let mut need_dot = false;
        if let Some(primary) = node {
            if matches!(primary.kind, K::NewClass { body: Some(_), .. }) {
                self.b.open(ZERO);
                self.scan(array_base(primary));
                self.token(".");
            } else {
                let mut is_text_block = false;
                let base = array_base(primary);
                if let K::Literal(LitKind::String) = base.kind {
                    is_text_block = self.source(primary).trim().ends_with("\"\"\"");
                }
                self.b.open_op(
                    OpenOp::new(self.plus_four.clone())
                        .name("visitDot")
                        .behaviour(if is_text_block { break_only_if_inner(false) } else { prefer_breaking_last(true) })
                        .breakability(if is_text_block {
                            LastLevelBreakability::AcceptInlineChain
                        } else {
                            LastLevelBreakability::AcceptInlineChainIfSimpleOtherwiseCheckInner
                        })
                        .column_limit(METHOD_CHAIN_COLUMN_LIMIT)
                        .simple(false),
                );
                self.scan(base);
                if !is_text_block {
                    self.b.break_op();
                }
                need_dot = true;
            }
            self.format_array_indices(array_indices(primary));
            if stack.is_empty() {
                self.b.close();
                return;
            }
        }
        let mut prefixes: Vec<usize> = Vec::new();
        let add_prefix = |p: &mut Vec<usize>, x: usize| {
            if !p.contains(&x) {
                p.push(x);
            }
        };
        if let Some(len) = type_names::type_prefix_length(&simple_names(&items)) {
            add_prefix(&mut prefixes, len);
        }
        let mut invocation_count = 0;
        let mut first_invocation_index: i64 = -1;
        for (i, e) in items.iter().enumerate() {
            if matches!(e.kind, K::MethodInvocation { .. }) {
                if i > 0 || node.is_some() {
                    invocation_count += 1;
                }
                if first_invocation_index < 0 {
                    first_invocation_index = i as i64;
                }
            }
        }
        if invocation_count == 1 && first_invocation_index > 0 {
            add_prefix(&mut prefixes, first_invocation_index as usize);
        }
        if prefixes.is_empty()
            && let K::Ident(name) = &items[0].kind
            && (name == "this" || name == "super")
        {
            add_prefix(&mut prefixes, 1);
        }
        let stream_prefixes: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, e)| match &e.kind {
                K::MethodInvocation { select, .. } => {
                    matches!(method_name(select), "stream" | "parallelStream" | "toBuilder")
                }
                _ => false,
            })
            .map(|(i, _)| i)
            .collect();
        for &p in &stream_prefixes {
            add_prefix(&mut prefixes, p);
        }
        if !prefixes.is_empty() {
            let fill = if stream_prefixes.is_empty() { FillMode::Independent } else { FillMode::Unified };
            self.visit_dot_with_prefix(&items, need_dot, &prefixes, fill);
        } else {
            self.visit_regular_dot(&items, need_dot);
        }
        if node.is_some() {
            self.b.close();
        }
    }

    fn visit_regular_dot(&mut self, items: &[&Tree], mut need_dot: bool) {
        let trailing_dereferences = items.len() > 1;
        let need_dot0 = need_dot;
        if !need_dot0 {
            self.b.open_op(
                OpenOp::new(self.plus_four.clone())
                    .name("visitRegularDot")
                    .behaviour(prefer_breaking_last(false))
                    .breakability(LastLevelBreakability::AcceptInlineChainIfSimpleOtherwiseCheckInner)
                    .column_limit(METHOD_CHAIN_COLUMN_LIMIT)
                    .simple(!trailing_dereferences),
            );
        }
        let min_length = (self.indent_multiplier * 4) as usize;
        let mut length = if need_dot0 { min_length } else { 0 };
        for e in items {
            if need_dot {
                if length > min_length {
                    self.b.break_doc(BreakDoc {
                        fill_mode: FillMode::Unified,
                        flat: "",
                        plus_indent: ZERO,
                        opt_tag: None,
                        has_column_limit: should_have_column_limit(e),
                    });
                }
                self.token(".");
                length += 1;
            }
            if !self.fill_first_argument(e, items, if trailing_dereferences { ZERO } else { self.minus_four.clone() }) {
                let tyarg_tag = self.b.new_tag();
                self.dot_expression_up_to_args(e, Some(tyarg_tag));
                let tyarg_indent = Indent::if_(tyarg_tag, self.plus_four.clone(), ZERO);
                self.dot_expression_args_and_paren(
                    e,
                    tyarg_indent,
                    if trailing_dereferences || need_dot { self.plus_four.clone() } else { ZERO },
                );
            }
            length += e.end - e.start;
            need_dot = true;
        }
        if !need_dot0 {
            self.b.close();
        }
    }

    fn fill_first_argument(&mut self, e: &Tree, items: &[&Tree], indent: Indent) -> bool {
        if items.len() < 2 {
            return false;
        }
        let K::MethodInvocation { select, type_args, args } = &e.kind else { return false };
        let K::Ident(name) = &select.kind else { return false };
        if name.len() > 4 || !type_args.is_empty() || args.len() != 1 {
            return false;
        }
        self.b.open(ZERO);
        self.b.open(indent);
        self.token(name);
        self.token("(");
        self.scan(&args[0]);
        self.b.close();
        self.token(")");
        self.b.close();
        true
    }

    fn visit_dot_with_prefix(
        &mut self,
        items: &[&Tree],
        mut need_dot: bool,
        prefixes: &[usize],
        prefix_fill: FillMode,
    ) {
        let trailing_dereferences = !prefixes.is_empty() && *prefixes.last().unwrap() < items.len() - 1;
        let has_method_invocations = items.iter().any(|e| matches!(e.kind, K::MethodInvocation { .. }));
        self.b.open_op(
            OpenOp::new(self.plus_four.clone())
                .name("visitDotWithPrefix")
                .behaviour(prefer_breaking_last(false))
                .breakability(if has_method_invocations {
                    LastLevelBreakability::AcceptInlineChainIfSimpleOtherwiseCheckInner
                } else {
                    LastLevelBreakability::CheckInner
                })
                .partial(PartialInlineability::IfFirstLevelFits)
                .column_limit(METHOD_CHAIN_COLUMN_LIMIT)
                .simple(!trailing_dereferences),
        );
        for _ in 0..prefixes.len() {
            self.b.open_op(OpenOp::new(ZERO).name("prefix"));
        }
        let mut sorted: Vec<usize> = prefixes.to_vec();
        sorted.sort_unstable();
        let mut unconsumed: VecDeque<usize> = sorted.into_iter().collect();
        let name_tag = self.b.new_tag();
        for (i, e) in items.iter().enumerate() {
            if need_dot {
                let fill_mode = match unconsumed.front() {
                    Some(&p) if i <= p => prefix_fill,
                    _ => FillMode::Unified,
                };
                self.b.break_doc(BreakDoc {
                    fill_mode,
                    flat: "",
                    plus_indent: ZERO,
                    opt_tag: Some(name_tag),
                    has_column_limit: should_have_column_limit(e),
                });
                self.token(".");
            }
            let tyarg_tag = self.b.new_tag();
            self.dot_expression_up_to_args(e, Some(tyarg_tag));
            if unconsumed.front() == Some(&i) {
                self.b.close();
                unconsumed.pop_front();
            }
            let tyarg_indent = Indent::if_(tyarg_tag, self.plus_four.clone(), ZERO);
            let args_indent = Indent::if_(
                name_tag,
                self.plus_four.clone(),
                if trailing_dereferences { self.plus_four.clone() } else { ZERO },
            );
            self.dot_expression_args_and_paren(e, tyarg_indent, args_indent);
            need_dot = true;
        }
        self.b.close();
    }

    fn dot_expression_up_to_args(&mut self, expression: &Tree, tyarg_tag: Option<BreakTag>) {
        let expression = array_base(expression);
        match &expression.kind {
            K::MemberSelect { name, .. } => self.token(name),
            K::MethodInvocation { select, type_args, .. } => {
                if !type_args.is_empty() {
                    self.b.open(self.plus_four.clone());
                    self.add_type_arguments(type_args, ZERO);
                    self.b.break_full(FillMode::Unified, "", ZERO, tyarg_tag);
                    self.b.close();
                }
                self.token(method_name(select));
            }
            K::Ident(name) => self.token(name),
            _ => self.scan(expression),
        }
    }

    fn dot_expression_args_and_paren(&mut self, expression: &Tree, tyarg_indent: Indent, indent: Indent) {
        let indices = array_indices(expression);
        let expression = array_base(expression);
        if let K::MethodInvocation { args, .. } = &expression.kind {
            self.b.open_op(
                OpenOp::new(tyarg_indent)
                    .behaviour(prefer_breaking_last(true))
                    .breakability(LastLevelBreakability::CheckInner)
                    .name("dotExpressionArgsAndParen"),
            );
            self.add_arguments(args, indent);
            self.b.close();
        }
        self.format_array_indices(indices);
    }

    fn format_array_indices(&mut self, mut indices: Vec<&Tree>) {
        if indices.is_empty() {
            return;
        }
        self.b.open(ZERO);
        while let Some(index) = indices.pop() {
            self.token("[");
            self.b.break_to_fill("");
            self.scan(index);
            self.token("]");
        }
        self.b.close();
    }

    fn add_type_arguments(&mut self, type_args: &[Tree], plus_indent: Indent) {
        if type_args.is_empty() {
            return;
        }
        self.token("<");
        self.b.open(plus_indent);
        let mut first = true;
        for t in type_args {
            if !first {
                self.token(",");
                self.b.break_to_fill(" ");
            }
            self.scan(t);
            first = false;
        }
        self.b.close();
        self.token(">");
    }

    fn add_arguments(&mut self, arguments: &[Tree], plus_indent: Indent) {
        self.b.open_op(
            OpenOp::new(plus_indent)
                .name("addArguments")
                .behaviour(prefer_breaking_last(false))
                .breakability(LastLevelBreakability::CheckInner)
                .simple(arguments.len() <= 1),
        );
        self.token("(");
        if !arguments.is_empty() {
            let args: Vec<&Tree> = arguments.iter().collect();
            if args.len().is_multiple_of(2) && self.arguments_are_tabular(&args) == 2 {
                self.b.forced_break();
                self.b.open(ZERO);
                let mut first = true;
                for pair in args.chunks(2) {
                    if !first {
                        self.token(",");
                        self.b.forced_break();
                    }
                    self.b.open(self.plus_four.clone());
                    self.scan(pair[0]);
                    self.token(",");
                    self.b.break_flat(" ");
                    self.scan(pair[1]);
                    self.b.close();
                    first = false;
                }
                self.b.close();
            } else if args.len() >= 2 && self.is_format_string(args[0]) {
                self.b.break_op();
                self.b.open(ZERO);
                self.scan(args[0]);
                self.token(",");
                self.b.break_flat(" ");
                self.b.open(ZERO);
                self.arg_list(&args[1..]);
                self.b.close();
                self.b.close();
            } else {
                self.b.break_op();
                self.arg_list(&args);
            }
        }
        self.token(")");
        self.b.close();
    }

    fn arg_list(&mut self, arguments: &[&Tree]) {
        self.b.open_op(
            OpenOp::new(ZERO)
                .name("argList")
                .behaviour(prefer_breaking_last(true))
                .breakability(LastLevelBreakability::CheckInner)
                .simple(arguments.len() <= 1),
        );
        let mut first = true;
        let fill_mode = if self.has_only_short_items(arguments) { FillMode::Independent } else { FillMode::Unified };
        for a in arguments {
            if !first {
                self.token(",");
                self.b.break_full(fill_mode, " ", ZERO, None);
            }
            self.scan(a);
            first = false;
        }
        self.b.close();
    }

    fn is_format_string(&self, first: &Tree) -> bool {
        let mut string_literal = true;
        let mut format_string = false;
        self.scan_format(first, &mut string_literal, &mut format_string);
        string_literal && format_string
    }

    fn scan_format(&self, t: &Tree, string_literal: &mut bool, format_string: &mut bool) {
        match &t.kind {
            K::Literal(LitKind::String) => {
                let s = self.source(t);
                if s.contains('%') || has_brace_digit(s) {
                    *format_string = true;
                }
            }
            K::Binary { op, lhs, rhs } if op == "+" => {
                self.scan_format(lhs, string_literal, format_string);
                self.scan_format(rhs, string_literal, format_string);
            }
            _ => *string_literal = false,
        }
    }
}

fn has_brace_digit(s: &str) -> bool {
    let b = s.as_bytes();
    b.windows(3).any(|w| w[0] == b'{' && w[1].is_ascii_digit() && w[2] == b'}')
}

fn directive_keyword(d: &Tree) -> &'static str {
    match &d.kind {
        K::Directive { keyword, .. } => keyword,
        _ => "",
    }
}

fn skip_paren(t: &Tree) -> &Tree {
    match &t.kind {
        K::Parens(e) => e,
        _ => t,
    }
}

fn is_array_value(arg: &Tree) -> bool {
    match &arg.kind {
        K::Assign { rhs, .. } => matches!(rhs.kind, K::NewArray { elem: None, .. }),
        _ => false,
    }
}

fn ambiguous_unary_operator(op: &str, expr: &Tree) -> bool {
    if op != "-" && op != "+" {
        return false;
    }
    match &expr.kind {
        K::Unary { op: inner, postfix: false, .. } => inner.starts_with(op),
        _ => false,
    }
}

fn walk_infix<'t>(prec: i32, t: &'t Tree, operands: &mut Vec<&'t Tree>, operators: &mut Vec<&'t str>) {
    if let K::Binary { op, lhs, rhs } = &t.kind
        && precedence(op) == prec
    {
        walk_infix(prec, lhs, operands, operators);
        operators.push(op);
        walk_infix(prec, rhs, operands, operators);
    } else {
        operands.push(t);
    }
}

fn is_string_concat(t: &Tree) -> bool {
    match &t.kind {
        K::Literal(LitKind::String) => true,
        K::Binary { op, lhs, rhs } if op == "+" => is_string_concat(lhs) || is_string_concat(rhs),
        _ => false,
    }
}

fn should_have_column_limit(e: &Tree) -> bool {
    matches!(array_base(e).kind, K::MethodInvocation { .. })
}

fn simple_names(items: &[&Tree]) -> Vec<String> {
    let mut names = Vec::new();
    for e in items {
        let is_array = matches!(e.kind, K::ArrayAccess { .. });
        let e = array_base(e);
        match &e.kind {
            K::MemberSelect { name, .. } => names.push(name.clone()),
            K::Ident(name) => names.push(name.clone()),
            K::MethodInvocation { select, .. } => {
                names.push(method_name(select).to_string());
                break;
            }
            _ => break,
        }
        if is_array {
            break;
        }
    }
    names
}

fn row_length(row: &[&Tree]) -> usize {
    let mut size = 0;
    for t in row {
        match &t.kind {
            K::NewArray { init: Some(init), .. } => {
                let refs: Vec<&Tree> = init.iter().collect();
                size += row_length(&refs);
            }
            _ => size += 1,
        }
    }
    size
}

fn expressions_are_parallel(rows: &[Vec<&Tree>], column: usize, at_least_m: usize) -> bool {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for row in rows {
        if column >= row.len() {
            continue;
        }
        *counts.entry(row[column].javac_kind()).or_default() += 1;
    }
    counts.values().any(|&c| c >= at_least_m)
}
