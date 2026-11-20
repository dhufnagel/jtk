//! A javac-shaped AST, built by the hand-written parser (`crate::parser`).
//!
//! The visitor is a port of `JavaInputAstVisitor`, which walks javac trees; a similar shape keeps it close to the
//! original. The linter checks the same trees.

use std::ops::Range;

pub type B = Box<Tree>;

#[derive(Debug, Clone)]
pub struct Tree {
    pub start: usize,
    pub end: usize,
    pub kind: K,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LitKind {
    Int,
    Long,
    Float,
    Double,
    Char,
    String,
    Boolean,
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    Class,
    Interface,
    Enum,
    Annotation,
    Record,
}

#[derive(Debug, Clone)]
pub struct Mods {
    pub start: usize,
    pub annotations: Vec<Tree>,
    /// The modifier keywords with their byte ranges, in source order.
    pub keywords: Vec<(Modifier, Range<usize>)>,
}

impl Mods {
    pub fn has(&self, modifier: Modifier) -> bool {
        self.keywords.iter().any(|(m, _)| *m == modifier)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Public,
    Protected,
    Private,
    Static,
    Abstract,
    Final,
    Native,
    Synchronized,
    Transient,
    Volatile,
    Strictfp,
    Default,
    Sealed,
    NonSealed,
}

impl Modifier {
    pub fn from_keyword(keyword: &str) -> Option<Modifier> {
        Some(match keyword {
            "public" => Modifier::Public,
            "protected" => Modifier::Protected,
            "private" => Modifier::Private,
            "static" => Modifier::Static,
            "abstract" => Modifier::Abstract,
            "final" => Modifier::Final,
            "native" => Modifier::Native,
            "synchronized" => Modifier::Synchronized,
            "transient" => Modifier::Transient,
            "volatile" => Modifier::Volatile,
            "strictfp" => Modifier::Strictfp,
            "default" => Modifier::Default,
            "sealed" => Modifier::Sealed,
            "non-sealed" => Modifier::NonSealed,
            _ => return None,
        })
    }

    pub fn keyword(self) -> &'static str {
        match self {
            Modifier::Public => "public",
            Modifier::Protected => "protected",
            Modifier::Private => "private",
            Modifier::Static => "static",
            Modifier::Abstract => "abstract",
            Modifier::Final => "final",
            Modifier::Native => "native",
            Modifier::Synchronized => "synchronized",
            Modifier::Transient => "transient",
            Modifier::Volatile => "volatile",
            Modifier::Strictfp => "strictfp",
            Modifier::Default => "default",
            Modifier::Sealed => "sealed",
            Modifier::NonSealed => "non-sealed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub start: usize,
    pub end: usize,
    pub mods: Mods,
    /// `None` for inferred lambda parameters and `var`.
    pub ty: Option<B>,
    pub name: String,
    pub init: Option<B>,
    pub is_record_component: bool,
    /// The offset of the name.
    pub name_pos: usize,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub kind: ClassKind,
    pub mods: Mods,
    pub name: String,
    pub type_params: Vec<Tree>,
    pub extends: Option<B>,
    pub implements: Vec<Tree>,
    pub permits: Vec<Tree>,
    pub record_components: Vec<VarDecl>,
    pub enum_constants: Vec<EnumConstant>,
    pub members: Vec<Tree>,
    /// The offset of the name.
    pub name_pos: usize,
}

#[derive(Debug, Clone)]
pub struct EnumConstant {
    pub mods: Mods,
    pub name: String,
    pub args: Vec<Tree>,
    pub body: Option<Vec<Tree>>,
    /// The offset of the name.
    pub name_pos: usize,
}

#[derive(Debug, Clone)]
pub struct MethodDecl {
    pub mods: Mods,
    /// Annotations appearing after the type parameters (they annotate the return type).
    pub return_type_annotations: Vec<Tree>,
    pub type_params: Vec<Tree>,
    /// `None` for constructors.
    pub ret: Option<B>,
    pub name: String,
    pub receiver: Option<Box<VarDecl>>,
    pub params: Vec<VarDecl>,
    pub throws: Vec<Tree>,
    pub body: Option<B>,
    pub default_value: Option<B>,
    pub compact_ctor: bool,
    /// The offset of the name.
    pub name_pos: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseKind {
    Statement,
    Rule,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub start: usize,
    pub labels: Vec<Tree>,
    pub is_default: bool,
    pub guard: Option<B>,
    pub kind: CaseKind,
    pub stmts: Vec<Tree>,
    pub body: Option<B>,
}

#[derive(Debug, Clone)]
pub struct Catch {
    pub start: usize,
    pub param: VarDecl,
    /// Union type alternatives, if more than one.
    pub union: Vec<Tree>,
    pub block: B,
}

#[derive(Debug, Clone)]
pub enum K {
    CompilationUnit {
        package: Option<(Vec<Tree>, B)>,
        imports: Vec<Tree>,
        types: Vec<Tree>,
        module: Option<B>,
    },
    Import {
        is_static: bool,
        name: B,
    },
    Class(Box<ClassDecl>),
    Method(Box<MethodDecl>),
    /// One or more variables declared together (fields and locals).
    Vars(Vec<VarDecl>),
    Block {
        is_static: bool,
        stmts: Vec<Tree>,
    },
    Empty,
    ExprStmt(B),
    If {
        cond: B,
        then: B,
        else_: Option<B>,
    },
    While {
        cond: B,
        body: B,
    },
    DoWhile {
        body: B,
        cond: B,
    },
    For {
        init: Vec<Tree>,
        cond: Option<B>,
        update: Vec<Tree>,
        body: B,
    },
    ForEach {
        var: Box<VarDecl>,
        expr: B,
        body: B,
    },
    Labeled {
        label: String,
        body: B,
    },
    Switch {
        selector: B,
        cases: Vec<Case>,
    },
    Try {
        resources: Vec<Tree>,
        block: B,
        catches: Vec<Catch>,
        finally: Option<B>,
    },
    Throw(B),
    Return(Option<B>),
    Break(Option<String>),
    Continue(Option<String>),
    Synchronized {
        lock: B,
        block: B,
    },
    Assert {
        cond: B,
        detail: Option<B>,
    },
    Yield(B),
    Var(Box<VarDecl>),
    // Expressions
    Ident(String),
    MemberSelect {
        expr: B,
        name: String,
    },
    MethodInvocation {
        select: B,
        type_args: Vec<Tree>,
        args: Vec<Tree>,
    },
    ArrayAccess {
        expr: B,
        index: B,
    },
    /// Like javac, `-` directly before a decimal integer literal is part of the literal (`-1`, `-2147483648L`):
    /// `2147483648` alone is no valid `int`. Other negations (`-1.0`, `-0x1`, `-x`) are `Unary`.
    Literal(LitKind),
    Binary {
        op: String,
        lhs: B,
        rhs: B,
    },
    Unary {
        op: String,
        postfix: bool,
        expr: B,
    },
    Assign {
        op: String,
        lhs: B,
        rhs: B,
    },
    Conditional {
        cond: B,
        then: B,
        else_: B,
    },
    InstanceOf {
        expr: B,
        target: B,
    },
    Cast {
        ty: B,
        expr: B,
    },
    Parens(B),
    NewClass {
        outer: Option<B>,
        type_args: Vec<Tree>,
        ident: B,
        args: Vec<Tree>,
        body: Option<Vec<Tree>>,
    },
    NewArray {
        elem: Option<B>,
        dims: Vec<Tree>,
        dim_annotations: Vec<Vec<Tree>>,
        init: Option<Vec<Tree>>,
    },
    Lambda {
        params: Vec<VarDecl>,
        body: B,
    },
    MemberRef {
        qual: B,
        type_args: Vec<Tree>,
        name: Option<String>,
    },
    SwitchExpr {
        selector: B,
        cases: Vec<Case>,
    },
    // Types
    Primitive(String),
    ArrayType {
        elem: B,
        dims: Vec<Vec<Tree>>,
    },
    Parameterized {
        base: B,
        args: Vec<Tree>,
    },
    Wildcard {
        bound: Option<(bool, B)>,
    },
    Annotated {
        annotations: Vec<Tree>,
        base: B,
    },
    Union(Vec<Tree>),
    Intersection(Vec<Tree>),
    Annotation {
        ty: B,
        args: Vec<Tree>,
    },
    TypeParameter {
        annotations: Vec<Tree>,
        name: String,
        bounds: Vec<Tree>,
    },
    // Patterns and case labels
    BindingPattern {
        var: Box<VarDecl>,
    },
    RecordPattern {
        ty: B,
        nested: Vec<Tree>,
    },
    /// The unnamed pattern `_` (Java 22).
    AnyPattern,
    DefaultCaseLabel,
    // Modules
    Module {
        annotations: Vec<Tree>,
        open: bool,
        name: B,
        directives: Vec<Tree>,
    },
    Directive {
        keyword: &'static str,
        name: B,
        separator: &'static str,
        items: Option<Vec<Tree>>,
    },
    Requires {
        name: B,
    },
    Uses {
        name: B,
    },
}

/// The variant of a [`K`], without its data: for dispatching on the kind of tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    CompilationUnit,
    Import,
    Class,
    Method,
    Vars,
    Block,
    Empty,
    ExprStmt,
    If,
    While,
    DoWhile,
    For,
    ForEach,
    Labeled,
    Switch,
    Try,
    Throw,
    Return,
    Break,
    Continue,
    Synchronized,
    Assert,
    Yield,
    Var,
    Ident,
    MemberSelect,
    MethodInvocation,
    ArrayAccess,
    Literal,
    Binary,
    Unary,
    Assign,
    Conditional,
    InstanceOf,
    Cast,
    Parens,
    NewClass,
    NewArray,
    Lambda,
    MemberRef,
    SwitchExpr,
    Primitive,
    ArrayType,
    Parameterized,
    Wildcard,
    Annotated,
    Union,
    Intersection,
    Annotation,
    TypeParameter,
    BindingPattern,
    RecordPattern,
    AnyPattern,
    DefaultCaseLabel,
    Module,
    Directive,
    Requires,
    Uses,
}

impl Tag {
    pub const COUNT: usize = Tag::Uses as usize + 1;
}

impl Tree {
    pub fn tag(&self) -> Tag {
        match self.kind {
            K::CompilationUnit { .. } => Tag::CompilationUnit,
            K::Import { .. } => Tag::Import,
            K::Class { .. } => Tag::Class,
            K::Method { .. } => Tag::Method,
            K::Vars { .. } => Tag::Vars,
            K::Block { .. } => Tag::Block,
            K::Empty => Tag::Empty,
            K::ExprStmt { .. } => Tag::ExprStmt,
            K::If { .. } => Tag::If,
            K::While { .. } => Tag::While,
            K::DoWhile { .. } => Tag::DoWhile,
            K::For { .. } => Tag::For,
            K::ForEach { .. } => Tag::ForEach,
            K::Labeled { .. } => Tag::Labeled,
            K::Switch { .. } => Tag::Switch,
            K::Try { .. } => Tag::Try,
            K::Throw { .. } => Tag::Throw,
            K::Return { .. } => Tag::Return,
            K::Break { .. } => Tag::Break,
            K::Continue { .. } => Tag::Continue,
            K::Synchronized { .. } => Tag::Synchronized,
            K::Assert { .. } => Tag::Assert,
            K::Yield { .. } => Tag::Yield,
            K::Var { .. } => Tag::Var,
            K::Ident { .. } => Tag::Ident,
            K::MemberSelect { .. } => Tag::MemberSelect,
            K::MethodInvocation { .. } => Tag::MethodInvocation,
            K::ArrayAccess { .. } => Tag::ArrayAccess,
            K::Literal { .. } => Tag::Literal,
            K::Binary { .. } => Tag::Binary,
            K::Unary { .. } => Tag::Unary,
            K::Assign { .. } => Tag::Assign,
            K::Conditional { .. } => Tag::Conditional,
            K::InstanceOf { .. } => Tag::InstanceOf,
            K::Cast { .. } => Tag::Cast,
            K::Parens { .. } => Tag::Parens,
            K::NewClass { .. } => Tag::NewClass,
            K::NewArray { .. } => Tag::NewArray,
            K::Lambda { .. } => Tag::Lambda,
            K::MemberRef { .. } => Tag::MemberRef,
            K::SwitchExpr { .. } => Tag::SwitchExpr,
            K::Primitive { .. } => Tag::Primitive,
            K::ArrayType { .. } => Tag::ArrayType,
            K::Parameterized { .. } => Tag::Parameterized,
            K::Wildcard { .. } => Tag::Wildcard,
            K::Annotated { .. } => Tag::Annotated,
            K::Union { .. } => Tag::Union,
            K::Intersection { .. } => Tag::Intersection,
            K::Annotation { .. } => Tag::Annotation,
            K::TypeParameter { .. } => Tag::TypeParameter,
            K::BindingPattern { .. } => Tag::BindingPattern,
            K::RecordPattern { .. } => Tag::RecordPattern,
            K::AnyPattern => Tag::AnyPattern,
            K::DefaultCaseLabel => Tag::DefaultCaseLabel,
            K::Module { .. } => Tag::Module,
            K::Directive { .. } => Tag::Directive,
            K::Requires { .. } => Tag::Requires,
            K::Uses { .. } => Tag::Uses,
        }
    }
}

impl Tree {
    pub fn new(start: usize, end: usize, kind: K) -> Tree {
        Tree { start, end, kind }
    }

    /// Calls `f` on each direct subtree, in source order. Annotations in modifiers, the types and initializers of
    /// variables and the parts of cases, catches, classes and methods are subtrees; names stored as strings are not.
    pub fn for_each_child<'t>(&'t self, f: &mut impl FnMut(&'t Tree)) {
        fn var<'t>(v: &'t VarDecl, f: &mut impl FnMut(&'t Tree)) {
            v.mods.annotations.iter().for_each(&mut *f);
            v.ty.iter().for_each(|t| f(t));
            v.init.iter().for_each(|t| f(t));
        }
        fn case<'t>(c: &'t Case, f: &mut impl FnMut(&'t Tree)) {
            c.labels.iter().for_each(&mut *f);
            c.guard.iter().for_each(|t| f(t));
            c.stmts.iter().for_each(&mut *f);
            c.body.iter().for_each(|t| f(t));
        }
        match &self.kind {
            K::CompilationUnit { package, imports, types, module } => {
                if let Some((annotations, name)) = package {
                    annotations.iter().for_each(&mut *f);
                    f(name);
                }
                imports.iter().for_each(&mut *f);
                types.iter().for_each(&mut *f);
                module.iter().for_each(|t| f(t));
            }
            K::Import { name, .. } => f(name),
            K::Class(c) => {
                c.mods.annotations.iter().for_each(&mut *f);
                c.type_params.iter().for_each(&mut *f);
                c.record_components.iter().for_each(|v| var(v, f));
                c.extends.iter().for_each(|t| f(t));
                c.implements.iter().for_each(&mut *f);
                c.permits.iter().for_each(&mut *f);
                for e in &c.enum_constants {
                    e.mods.annotations.iter().for_each(&mut *f);
                    e.args.iter().for_each(&mut *f);
                    e.body.iter().flatten().for_each(&mut *f);
                }
                c.members.iter().for_each(&mut *f);
            }
            K::Method(m) => {
                m.mods.annotations.iter().for_each(&mut *f);
                m.type_params.iter().for_each(&mut *f);
                m.return_type_annotations.iter().for_each(&mut *f);
                m.ret.iter().for_each(|t| f(t));
                m.receiver.iter().for_each(|v| var(v, f));
                m.params.iter().for_each(|v| var(v, f));
                m.throws.iter().for_each(&mut *f);
                m.body.iter().for_each(|t| f(t));
                m.default_value.iter().for_each(|t| f(t));
            }
            K::Vars(vars) => vars.iter().for_each(|v| var(v, f)),
            K::Var(v) => var(v, f),
            K::Block { stmts, .. } => stmts.iter().for_each(&mut *f),
            K::Empty
            | K::Ident(_)
            | K::Literal(_)
            | K::Primitive(_)
            | K::AnyPattern
            | K::DefaultCaseLabel
            | K::Break(_)
            | K::Continue(_) => {}
            K::ExprStmt(e) | K::Throw(e) | K::Yield(e) | K::Parens(e) => f(e),
            K::If { cond, then, else_ } => {
                f(cond);
                f(then);
                else_.iter().for_each(|t| f(t));
            }
            K::While { cond, body } => {
                f(cond);
                f(body);
            }
            K::DoWhile { body, cond } => {
                f(body);
                f(cond);
            }
            K::For { init, cond, update, body } => {
                init.iter().for_each(&mut *f);
                cond.iter().for_each(|t| f(t));
                update.iter().for_each(&mut *f);
                f(body);
            }
            K::ForEach { var: v, expr, body } => {
                var(v, f);
                f(expr);
                f(body);
            }
            K::Labeled { body, .. } => f(body),
            K::Switch { selector, cases } | K::SwitchExpr { selector, cases } => {
                f(selector);
                cases.iter().for_each(|c| case(c, f));
            }
            K::Try { resources, block, catches, finally } => {
                resources.iter().for_each(&mut *f);
                f(block);
                for c in catches {
                    var(&c.param, f);
                    f(&c.block);
                }
                finally.iter().for_each(|t| f(t));
            }
            K::Return(e) => e.iter().for_each(|t| f(t)),
            K::Synchronized { lock, block } => {
                f(lock);
                f(block);
            }
            K::Assert { cond, detail } => {
                f(cond);
                detail.iter().for_each(|t| f(t));
            }
            K::MemberSelect { expr, .. } => f(expr),
            K::MethodInvocation { select, type_args, args } => {
                f(select);
                type_args.iter().for_each(&mut *f);
                args.iter().for_each(&mut *f);
            }
            K::ArrayAccess { expr, index } => {
                f(expr);
                f(index);
            }
            K::Binary { lhs, rhs, .. } | K::Assign { lhs, rhs, .. } => {
                f(lhs);
                f(rhs);
            }
            K::Unary { expr, .. } => f(expr),
            K::Conditional { cond, then, else_ } => {
                f(cond);
                f(then);
                f(else_);
            }
            K::InstanceOf { expr, target } => {
                f(expr);
                f(target);
            }
            K::Cast { ty, expr } => {
                f(ty);
                f(expr);
            }
            K::NewClass { outer, type_args, ident, args, body } => {
                outer.iter().for_each(|t| f(t));
                type_args.iter().for_each(&mut *f);
                f(ident);
                args.iter().for_each(&mut *f);
                body.iter().flatten().for_each(&mut *f);
            }
            K::NewArray { elem, dims, dim_annotations, init } => {
                elem.iter().for_each(|t| f(t));
                dims.iter().for_each(&mut *f);
                dim_annotations.iter().flatten().for_each(&mut *f);
                init.iter().flatten().for_each(&mut *f);
            }
            K::Lambda { params, body } => {
                params.iter().for_each(|v| var(v, f));
                f(body);
            }
            K::MemberRef { qual, type_args, .. } => {
                f(qual);
                type_args.iter().for_each(&mut *f);
            }
            K::ArrayType { elem, dims } => {
                f(elem);
                dims.iter().flatten().for_each(&mut *f);
            }
            K::Parameterized { base, args } => {
                f(base);
                args.iter().for_each(&mut *f);
            }
            K::Wildcard { bound } => bound.iter().for_each(|(_, t)| f(t)),
            K::Annotated { annotations, base } => {
                annotations.iter().for_each(&mut *f);
                f(base);
            }
            K::Union(types) | K::Intersection(types) => types.iter().for_each(&mut *f),
            K::Annotation { ty, args } => {
                f(ty);
                args.iter().for_each(&mut *f);
            }
            K::TypeParameter { annotations, bounds, .. } => {
                annotations.iter().for_each(&mut *f);
                bounds.iter().for_each(&mut *f);
            }
            K::BindingPattern { var: v } => var(v, f),
            K::RecordPattern { ty, nested } => {
                f(ty);
                nested.iter().for_each(&mut *f);
            }
            K::Module { annotations, name, directives, .. } => {
                annotations.iter().for_each(&mut *f);
                f(name);
                directives.iter().for_each(&mut *f);
            }
            K::Directive { name, items, .. } => {
                f(name);
                items.iter().flatten().for_each(&mut *f);
            }
            K::Requires { name } | K::Uses { name } => f(name),
        }
    }

    pub fn is_expression(&self) -> bool {
        matches!(
            self.kind,
            K::Ident(_)
                | K::MemberSelect { .. }
                | K::MethodInvocation { .. }
                | K::ArrayAccess { .. }
                | K::Literal(_)
                | K::Binary { .. }
                | K::Unary { .. }
                | K::Assign { .. }
                | K::Conditional { .. }
                | K::InstanceOf { .. }
                | K::Cast { .. }
                | K::Parens(_)
                | K::NewClass { .. }
                | K::NewArray { .. }
                | K::Lambda { .. }
                | K::MemberRef { .. }
                | K::SwitchExpr { .. }
                | K::Annotated { .. }
                | K::Annotation { .. }
        )
    }

    /// The javac `Tree.Kind`, as used by heuristics comparing node kinds.
    pub fn javac_kind(&self) -> String {
        match &self.kind {
            K::Literal(l) => format!("{l:?}_LITERAL"),
            K::Binary { op, .. } => format!("BINARY{op}"),
            K::Unary { op, postfix, .. } => format!("UNARY{op}{postfix}"),
            K::Assign { op, .. } => format!("ASSIGN{op}"),
            other => {
                let s = format!("{other:?}");
                s.split(|c: char| !c.is_alphanumeric()).next().unwrap_or("").to_string()
            }
        }
    }
}

/// Drops trees without recursion through the kinds that nest deeply in real code (chains of operators, calls and
/// `else if`, nested blocks): the default, recursive drop overflows the stack on a chain of thousands of `+`.
impl Drop for Tree {
    fn drop(&mut self) {
        if !has_nesting_children(&self.kind) {
            return;
        }
        let mut stack = Vec::new();
        take_nesting_children(&mut self.kind, &mut stack);
        while let Some(mut tree) = stack.pop() {
            take_nesting_children(&mut tree.kind, &mut stack);
        }
    }
}

impl Tree {
    /// Takes the kind out of the tree, leaving an empty statement.
    pub fn take_kind(&mut self) -> K {
        std::mem::replace(&mut self.kind, K::Empty)
    }
}

fn has_nesting_children(kind: &K) -> bool {
    match kind {
        K::Block { stmts, .. } => !stmts.is_empty(),
        K::Return(expr) => expr.is_some(),
        _ => matches!(
            kind,
            K::Binary { .. }
                | K::Assign { .. }
                | K::ArrayAccess { .. }
                | K::Unary { .. }
                | K::Parens(_)
                | K::ExprStmt(_)
                | K::MemberSelect { .. }
                | K::Cast { .. }
                | K::InstanceOf { .. }
                | K::Throw(_)
                | K::Yield(_)
                | K::Lambda { .. }
                | K::Conditional { .. }
                | K::If { .. }
                | K::MethodInvocation { .. }
                | K::NewClass { .. }
        ),
    }
}

/// Moves the subtrees of the kinds that nest deeply onto `stack`, leaving empty trees in their place.
fn take_nesting_children(kind: &mut K, stack: &mut Vec<Tree>) {
    fn take(tree: &mut B, stack: &mut Vec<Tree>) {
        if !matches!(tree.kind, K::Empty) {
            stack.push(std::mem::replace(&mut **tree, Tree::new(0, 0, K::Empty)));
        }
    }
    match kind {
        K::Binary { lhs, rhs, .. } | K::Assign { lhs, rhs, .. } => {
            take(lhs, stack);
            take(rhs, stack);
        }
        K::ArrayAccess { expr, index } => {
            take(expr, stack);
            take(index, stack);
        }
        K::Unary { expr, .. }
        | K::Parens(expr)
        | K::ExprStmt(expr)
        | K::MemberSelect { expr, .. }
        | K::Cast { expr, .. }
        | K::InstanceOf { expr, .. }
        | K::Return(Some(expr))
        | K::Throw(expr)
        | K::Yield(expr)
        | K::Lambda { body: expr, .. } => take(expr, stack),
        K::Conditional { cond, then, else_ } => {
            take(cond, stack);
            take(then, stack);
            take(else_, stack);
        }
        K::If { cond, then, else_ } => {
            take(cond, stack);
            take(then, stack);
            if let Some(else_) = else_ {
                take(else_, stack);
            }
        }
        K::MethodInvocation { select, args, .. } => {
            take(select, stack);
            stack.append(args);
        }
        K::NewClass { outer, args, .. } => {
            if let Some(outer) = outer {
                take(outer, stack);
            }
            stack.append(args);
        }
        K::Block { stmts, .. } => stack.append(stmts),
        _ => {}
    }
}
